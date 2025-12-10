import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core'; // Import Invoke
import { Light as SyntaxHighlighter } from 'react-syntax-highlighter';
import { atomOneDark } from 'react-syntax-highlighter/dist/esm/styles/hljs';
import './App.css';

function App() {
  const [view, setView] = useState('search');
  const [query, setQuery] = useState('');
  const [results, setResults] = useState([]);
  
  const [formData, setFormData] = useState({
    problem: '', solution: '', explanation: '', tags: ''
  });
  const [message, setMessage] = useState('');

  // --- HÀM TÌM KIẾM (GỌI RUST) ---
  const handleSearch = async (e) => {
    e.preventDefault();
    if (!query) return;
    try {
      // Gọi hàm 'search_note' trong Rust
      const res = await invoke('search_note', { query: query });
      setResults(res);
    } catch (error) {
      console.error("Lỗi tìm kiếm:", error);
    }
  };

  // --- HÀM THÊM MỚI (GỌI RUST) ---
  const handleAdd = async (e) => {
    e.preventDefault();
    try {
      // Gọi hàm 'add_note' trong Rust
      await invoke('add_note', {
        problem: formData.problem,
        solution: formData.solution,
        explanation: formData.explanation,
        tags: formData.tags
      });

      setMessage("Đã nạp kiến thức thành công!");
      setFormData({ problem: '', solution: '', explanation: '', tags: '' });
      setTimeout(() => setMessage(''), 3000);
    } catch (error) {
      setMessage("Lỗi: " + error);
    }
  };

  return (
    <div className="container">
      <nav className="navbar">
        <button className={view === 'search' ? 'active' : ''} onClick={() => setView('search')}>🔍 Tìm kiếm</button>
        <button className={view === 'add' ? 'active' : ''} onClick={() => setView('add')}>➕ Nạp kiến thức</button>
      </nav>

      {view === 'search' && (
        <div className="search-view">
          <form onSubmit={handleSearch} className="search-box">
            <input type="text" placeholder="Bạn đang gặp vấn đề gì?" value={query} onChange={(e) => setQuery(e.target.value)} autoFocus />
            <button type="submit">Tìm</button>
          </form>

          <div className="results-list">
            {results.map((item) => (
              <div key={item.id} className="note-card">
                <div className="note-header"><h3>{item.problem}</h3></div>
                <div className="note-explanation"><strong>💡 Tại sao:</strong> {item.explanation}</div>
                <div className="note-code">
                  <SyntaxHighlighter language="python" style={atomOneDark}>{item.solution}</SyntaxHighlighter>
                </div>
                <div className="note-tags">
                  {item.tags.split(',').map(tag => <span key={tag} className="tag">#{tag}</span>)}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {view === 'add' && (
        <div className="add-view">
          <h2>Ghi chép kiến thức mới</h2>
          {message && <p className="status-msg">{message}</p>}
          <form onSubmit={handleAdd}>
            <div className="form-group"><label>1. Vấn đề</label><input type="text" value={formData.problem} onChange={e => setFormData({...formData, problem: e.target.value})} required /></div>
            <div className="form-group"><label>2. Giải pháp (Code)</label><textarea rows="5" value={formData.solution} onChange={e => setFormData({...formData, solution: e.target.value})} required className="code-input" /></div>
            <div className="form-group"><label>3. Giải thích</label><textarea rows="3" value={formData.explanation} onChange={e => setFormData({...formData, explanation: e.target.value})} required /></div>
            <div className="form-group"><label>Tags</label><input type="text" value={formData.tags} onChange={e => setFormData({...formData, tags: e.target.value})} /></div>
            <button type="submit" className="save-btn">Lưu</button>
          </form>
        </div>
      )}
    </div>
  );
}

export default App;