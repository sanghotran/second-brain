use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
struct Note {
    id: String,
    problem: String,
    solution: String,
    explanation: String,
    tags: String,
}

struct AppState {
    db: Mutex<Connection>,
    model: Mutex<TextEmbedding>,
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
        )",
        [],
    ).expect("Failed to create table");
    conn
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot_product / (norm_a * norm_b) }
}

#[tauri::command]
fn add_note(state: State<AppState>, problem: String, solution: String, explanation: String, tags: String) -> Result<String, String> {
    let content = format!("Problem: {}\nSolution: {}\nExplanation: {}", problem, solution, explanation);
    
    // --- KHÁC BIỆT CỦA BẢN 2.12.0 ---
    let model = state.model.lock().map_err(|_| "Failed to lock model")?;
    // Bản cũ embed trả về Vec<Vec<f32>> luôn, không cần unwrap phức tạp
    let embeddings = model.embed(vec![content], None).map_err(|e| e.to_string())?;
    let vector = &embeddings[0]; 
    // --------------------------------

    let vector_json = serde_json::to_string(vector).map_err(|e| e.to_string())?;
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
    let model = state.model.lock().map_err(|_| "Failed to lock model")?;
    let query_embeddings = model.embed(vec![query], None).map_err(|e| e.to_string())?;
    let query_vector = &query_embeddings[0];

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
            let score = cosine_similarity(query_vector, &vector);
            if score > 0.4 { results.push((score, note)); }
        }
    }
    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    Ok(results.into_iter().take(5).map(|(_, note)| note).collect())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Bản 2.12.0 API cũng như này, vẫn chạy tốt
    let model = TextEmbedding::try_new(InitOptions {
        model_name: EmbeddingModel::AllMiniLML6V2, 
        show_download_progress: true,
        ..Default::default()
    }).expect("Failed to initialize AI Model");

    let db = init_db();

    tauri::Builder::default()
        .manage(AppState { db: Mutex::new(db), model: Mutex::new(model) })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![add_note, search_note])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}