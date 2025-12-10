use anyhow::Error as E;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{api::sync::Api, Repo, RepoType};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;
use tokenizers::{PaddingParams, Tokenizer}; // Thêm PaddingParams
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
struct Note {
    id: String,
    problem: String,
    solution: String,
    explanation: String,
    tags: String,
}

struct MyModel {
    model: BertModel,
    tokenizer: Tokenizer,
}

impl MyModel {
    fn new() -> Result<Self, E> {
        let device = Device::Cpu;
        let api = Api::new()?;
        let repo = api.repo(Repo::with_revision(
            "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            RepoType::Model,
            "main".to_string(),
        ));

        let config_filename = repo.get("config.json")?;
        let tokenizer_filename = repo.get("tokenizer.json")?;
        let weights_filename = repo.get("model.safetensors")?;

        let config: Config = serde_json::from_str(&std::fs::read_to_string(config_filename)?)?;
        let mut tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(E::msg)?;

        // CẤU HÌNH TOKENIZER CHUẨN ĐỂ KHÔNG BỊ SAI SỐ
        if let Some(pp) = tokenizer.get_padding_mut() {
            pp.strategy = tokenizers::PaddingStrategy::BatchLongest
        } else {
            let pp = PaddingParams {
                strategy: tokenizers::PaddingStrategy::BatchLongest,
                ..Default::default()
            };
            tokenizer.with_padding(Some(pp));
        }

        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights_filename], DTYPE, &device)? };
        let model = BertModel::load(vb, &config)?;

        Ok(Self { model, tokenizer })
    }

    fn embed(&mut self, text: &str) -> Result<Vec<f32>, E> {
        let device = Device::Cpu;
        
        // 1. Encode text lấy cả ID và Attention Mask
        let tokens = self.tokenizer.encode(text, true).map_err(E::msg)?;
        
        // Lấy Input IDs (Token thật)
        let token_ids = Tensor::new(tokens.get_ids(), &device)?.unsqueeze(0)?;
        
        // Lấy Attention Mask (Để biết đâu là từ thật, đâu là khoảng trắng đệm)
        let attention_mask = Tensor::new(tokens.get_attention_mask(), &device)?.unsqueeze(0)?;

        // Tạo Token Type IDs (Mặc định là 0 hết cho BERT)
        let token_type_ids = token_ids.zeros_like()?;

        // 2. Chạy Model (Forward Pass)
        let embeddings = self.model.forward(&token_ids, &token_type_ids, Some(&attention_mask))?;

        // 3. THUẬT TOÁN "MASKED MEAN POOLING" (GIỐNG HỆT PYTHON)
        // Lấy chiều kích thước
        let (_batch_size, _seq_len, hidden_size) = embeddings.dims3()?;
        
        // Mở rộng mask để khớp với kích thước embedding
        // Mask gốc: [1, Seq_Len] -> Mở rộng thành [1, Seq_Len, Hidden_Size]
        let mask = attention_mask.unsqueeze(2)?.broadcast_as((1, _seq_len, hidden_size))?.to_dtype(DTYPE)?;

        // Nhân Embedding với Mask (Để triệt tiêu các token vô nghĩa về 0)
        let masked_embeddings = (embeddings * &mask)?;

        // Tính tổng các vector (chỉ những từ thật)
        let sum_embeddings = masked_embeddings.sum(1)?; // Cộng dồn theo chiều dọc câu

        // Tính tổng số lượng từ thật (Sum của mask)
        let sum_mask = mask.sum(1)?;
        
        // Chia trung bình (Đây mới là vector chuẩn của câu)
        // Dùng clamp để tránh chia cho 0
        let pooled_output = (sum_embeddings / sum_mask.clamp(1e-9, f64::MAX)?)?;
        let pooled_output = pooled_output.get(0)?; // Lấy vector đầu tiên

        // 4. Normalize (Chuẩn hóa vector về độ dài 1 - để tính Cosine chuẩn xác)
        let sum_sq: f32 = pooled_output.sqr()?.sum_all()?.to_scalar()?;
        let normalized = (pooled_output / (sum_sq.sqrt() as f64))?;

        Ok(normalized.to_vec1()?)
    }
}

struct AppState {
    db: Mutex<Connection>,
    model: Mutex<MyModel>,
}

fn init_db() -> Connection {
    let conn = Connection::open("brain.db").expect("Failed to open DB");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            problem TEXT,
            solution TEXT,
            explanation TEXT,
            tags TEXT,
            vector TEXT
        )", [],
    ).expect("Failed to create table");
    conn
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    dot_product
}

#[tauri::command]
fn add_note(state: State<AppState>, problem: String, solution: String, explanation: String, tags: String) -> Result<String, String> {
    let content = format!("Problem: {}\nSolution: {}\nExplanation: {}", problem, solution, explanation);
    let mut model_guard = state.model.lock().map_err(|_| "Failed to lock model")?;
    let vector = model_guard.embed(&content).map_err(|e| e.to_string())?;
    
    let vector_json = serde_json::to_string(&vector).map_err(|e| e.to_string())?;
    let conn = state.db.lock().map_err(|_| "Failed to lock db")?;
    let note_id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO notes (id, problem, solution, explanation, tags, vector) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![note_id, problem, solution, explanation, tags, vector_json],
    ).map_err(|e| e.to_string())?;

    Ok(note_id)
}

#[tauri::command]
fn search_note(state: State<AppState>, query: String) -> Result<Vec<Note>, String> {
    let mut model_guard = state.model.lock().map_err(|_| "Failed to lock model")?;
    let query_vector = model_guard.embed(&query).map_err(|e| e.to_string())?;

    let conn = state.db.lock().map_err(|_| "Failed to lock db")?;
    let mut stmt = conn.prepare("SELECT id, problem, solution, explanation, tags, vector FROM notes").map_err(|e| e.to_string())?;
    
    let mut results = Vec::new();
    let rows = stmt.query_map([], |row| {
        let vector_str: String = row.get(5)?;
        let vector: Vec<f32> = serde_json::from_str(&vector_str).unwrap_or_default();
        Ok((
            Note {
                id: row.get(0)?,
                problem: row.get(1)?,
                solution: row.get(2)?,
                explanation: row.get(3)?,
                tags: row.get(4)?,
            },
            vector
        ))
    }).map_err(|e| e.to_string())?;

    for row in rows {
        if let Ok((note, vector)) = row {
            let score = cosine_similarity(&query_vector, &vector);
            // Giữ ngưỡng 0.4 là hợp lý khi thuật toán đã chuẩn
            if score > 0.35 { results.push((score, note)); }
        }
    }
    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    Ok(results.into_iter().take(5).map(|(_, note)| note).collect())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let model = MyModel::new().expect("Failed to initialize Candle AI");
    let db = init_db();

    tauri::Builder::default()
        .manage(AppState { db: Mutex::new(db), model: Mutex::new(model) })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![add_note, search_note])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}