use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;
use uuid::Uuid;

// Cấu trúc dữ liệu trả về cho Frontend
#[derive(Serialize, Deserialize, Debug)]
struct Note {
    id: String,
    problem: String,
    solution: String,
    explanation: String,
    tags: String,
}

// Trạng thái chia sẻ giữa các luồng (DB và Model AI)
struct AppState {
    db: Mutex<Connection>,
    model: Mutex<TextEmbedding>,
}

// 1. Khởi tạo Database (Tạo file brain.db và bảng)
fn init_db() -> Connection {
    let conn = Connection::open("brain.db").expect("Failed to open DB");
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            problem TEXT,
            solution TEXT,
            explanation TEXT,
            tags TEXT,
            vector TEXT -- Lưu vector dưới dạng chuỗi JSON
        )",
        [],
    ).expect("Failed to create table");
    
    conn
}

// 2. Hàm tính độ tương đồng Cosine (Logic cốt lõi của RAG)
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot_product / (norm_a * norm_b) }
}

// --- COMMAND: THÊM GHI CHÚ ---
#[tauri::command]
fn add_note(state: State<AppState>, problem: String, solution: String, explanation: String, tags: String) -> Result<String, String> {
    // 1. Gộp nội dung để AI đọc
    let content = format!("Problem: {}\nSolution: {}\nExplanation: {}", problem, solution, explanation);

    // 2. Lock model để xử lý
    let model = state.model.lock().map_err(|_| "Failed to lock model")?;
    
    // 3. Tạo Vector (Embed)
    let embeddings = model.embed(vec![content], None).map_err(|e| e.to_string())?;
    let vector = &embeddings[0]; // Lấy vector đầu tiên

    // 4. Serialize Vector sang chuỗi JSON để lưu vào SQLite
    let vector_json = serde_json::to_string(vector).map_err(|e| e.to_string())?;

    // 5. Lưu vào DB
    let conn = state.db.lock().map_err(|_| "Failed to lock db")?;
    let note_id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO notes (id, problem, solution, explanation, tags, vector) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![note_id, problem, solution, explanation, tags, vector_json],
    ).map_err(|e| e.to_string())?;

    Ok(note_id)
}

// --- COMMAND: TÌM KIẾM ---
#[tauri::command]
fn search_note(state: State<AppState>, query: String) -> Result<Vec<Note>, String> {
    // 1. Tạo Vector cho câu hỏi
    let model = state.model.lock().map_err(|_| "Failed to lock model")?;
    let query_embeddings = model.embed(vec![query], None).map_err(|e| e.to_string())?;
    let query_vector = &query_embeddings[0];

    // 2. Lấy dữ liệu từ DB
    let conn = state.db.lock().map_err(|_| "Failed to lock db")?;
    let mut stmt = conn.prepare("SELECT id, problem, solution, explanation, tags, vector FROM notes").map_err(|e| e.to_string())?;
    
    let mut results = Vec::new();

    // 3. Duyệt qua từng note và tính điểm giống nhau
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
            let score = cosine_similarity(query_vector, &vector);
            if score > 0.4 { // Chỉ lấy kết quả giống > 40%
                results.push((score, note));
            }
        }
    }

    // 4. Sắp xếp điểm cao nhất lên đầu
    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    // Trả về top 5 kết quả
    Ok(results.into_iter().take(5).map(|(_, note)| note).collect())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Tải Model AI khi khởi động
    let model = TextEmbedding::try_new(InitOptions {
        model_name: EmbeddingModel::AllMiniLML6V2, 
        show_download_progress: true,
        ..Default::default()
    }).expect("Failed to initialize AI Model");

    let db = init_db();

    tauri::Builder::default()
        // Đăng ký State
        .manage(AppState { 
            db: Mutex::new(db), 
            model: Mutex::new(model) 
        })
        .plugin(tauri_plugin_shell::init())
        // Đăng ký Command cho Frontend gọi
        .invoke_handler(tauri::generate_handler![add_note, search_note])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}