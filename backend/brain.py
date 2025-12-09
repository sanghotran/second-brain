import chromadb
from sentence_transformers import SentenceTransformer
import uuid
import os

class PersonalBrain:
    def __init__(self):
        # init vector db
        db_path = os.path.join(os.path.dirname(__file__), "knowledge_db")
        self.chroma_client = chromadb.PersistentClient(path=db_path)
        
        # create collection
        self.collection = self.chroma_client.get_or_create_collection(
            name="my_notes"
        )

        # load model all-MiniLM-L6-V2
        print("Loading model...")
        self.embed_model = SentenceTransformer("all-MiniLM-L6-v2")
        print("Model loaded.")

    def add_note(self, problem: str, solution: str, explanation: str, tags: list):
        # create id
        note_id = str(uuid.uuid4())

        # full content for AI
        full_content_for_ai = f"Vấn đề: {problem}\nGiải pháp: {solution}\nGiải thích chi tiết: {explanation}"

        # create vector
        vector = self.embed_model.encode(full_content_for_ai).tolist()
        
        # save to chromedb
        self.collection.add(
            ids=[note_id],
            documents=[full_content_for_ai],
            embeddings=[vector],
            metadatas=[{
                "problem": problem,
                "solution": solution,
                "explanation": explanation,
                "tags": ",".join(tags),
                "type": "code_snippet"
            }]
        )
        return note_id

    def search(self, query: str, limit: int = 5):
        # change query to vector
        query_vector = self.embed_model.encode(query).tolist()
        
        # search in db
        results = self.collection.query(
            query_embeddings=[query_vector],
            n_results=limit
        )

        # clean results
        cleaned_results = []
        if results["ids"]:
            for i in range(len(results['ids'][0])):
                cleaned_results.append({
                    "id": results["ids"][0][i],
                    "content": results["documents"][0][i],
                    "metadata": results["metadatas"][0][i],
                    "score": results["distances"][0][i] # lower value is better
                })
        
        return cleaned_results
