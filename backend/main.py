from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from typing import List, Optional
from brain import PersonalBrain

app = FastAPI()

my_brain = PersonalBrain()

class NoteInput(BaseModel):
    problem: str
    solution: str
    explanation: str
    tags: Optional[List[str]] = []

    
@app.get("/")
def read_root():
    return {"message": "Second Brain API is running."}

@app.post("/add")
def add_new_note(note: NoteInput):
    try:
        note_id = my_brain.add_note(
            problem=note.problem,
            solution=note.solution,
            explanation=note.explanation,
            tags=note.tags
        )
        return {"status": "success", "id": note_id, "message": "Note added successfully."}

    except Exception as e:
        raise HTTPException(status_code=500, detail="Failed to add note.") from e

@app.get("/search")
def search_notes(query: str):
    results = my_brain.search(query)
    return {"results": results}

from fastapi.middleware.cors import CORSMiddleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"], # allow all origins
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# ... (Code cũ giữ nguyên)

# Thêm đoạn này vào cuối file main.py
if __name__ == "__main__":
    import uvicorn
    import multiprocessing
    
    # Fix lỗi multiprocessing khi đóng gói thành exe trên Windows
    multiprocessing.freeze_support() 
    
    # Chạy server (tắt reload vì exe không reload được)
    uvicorn.run(app, host="127.0.0.1", port=8000, reload=False)