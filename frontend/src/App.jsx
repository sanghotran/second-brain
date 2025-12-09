import { useState } from 'react';
import axios from 'axios';
import { Light as SyntaxHighlighter } from 'react-syntax-highlighter';
import { atomOneDark } from 'react-syntax-highlighter/dist/esm/styles/hljs';
import './App.css';

const API_URL = "http://127.0.0.1:8000";

// Trong file App.jsx

useEffect(() => {
  const startBackend = async () => {
    try {
      // 1. Gọi lệnh chạy file exe Python ngầm (Sidecar)
      // Lưu ý: 'binaries/backend' là đường dẫn ảo, Tauri tự map với file thực tế
      const command = Command.sidecar('binaries/backend');
      const child = await command.spawn();
      console.log('Backend started with PID:', child.pid);
      
      // 2. (Mới thêm) Ping liên tục cho đến khi Server sống
      checkHealth();
    } catch (err) {
      console.error('Lỗi khởi động Backend:', err);
    }
  };

  const checkHealth = async () => {
    let retries = 10;
    while (retries > 0) {
      try {
        // Gọi thử API root để xem sống chưa
        await axios.get('http://127.0.0.1:8000/');
        console.log("Backend đã sẵn sàng!");
        return; // Thoát vòng lặp
      } catch (e) {
        console.log("Đang đợi Backend... " + retries);
        await new Promise(r => setTimeout(r, 1000)); // Đợi 1 giây
        retries--;
      }
    }
    alert("Không thể kết nối Backend sau 10 giây. Hãy khởi động lại App!");
  };

  startBackend();
}, []);

function App() {
  const [view, setView] = useState('search'); // 'search' hoặc 'add'
  
  // --- STATE CHO TÌM KIẾM ---
  const [query, setQuery] = useState('');
  const [results, setResults] = useState([]);

  // --- STATE CHO THÊM MỚI ---
  const [formData, setFormData] = useState({
    problem: '',
    solution: '',
    explanation: '',
    tags: ''
  });
  const [message, setMessage] = useState('');
  // --- THÊM ĐOẠN NÀY ĐỂ CHẠY PYTHON ---
  useEffect(() => {
    // Hàm khởi động Sidecar
    const startBackend = async () => {
      try {
        console.log("Đang khởi động Brain Engine...");
        // 'backend' phải khớp với tên trong tauri.conf.json
        const command = Command.sidecar('binaries/backend');
        const child = await command.spawn();
        console.log('Brain Engine PID:', child.pid);
      } catch (err) {
        console.error('Không thể khởi động Backend:', err);
      }
    };

    startBackend();
  }, []);
  // ------------------------------------

  // --- XỬ LÝ TÌM KIẾM ---
  const handleSearch = async (e) => {
    e.preventDefault();
    if (!query) return;
    try {
      const res = await axios.get(`${API_URL}/search`, { params: { query } });
      setResults(res.data.results);
    } catch (error) {
      console.error("Lỗi tìm kiếm:", error);
    }
  };

  // --- XỬ LÝ THÊM MỚI ---
  const handleAdd = async (e) => {
    e.preventDefault();
    try {
      // Tách tags từ chuỗi "tag1, tag2" thành array
      const tagsArray = formData.tags.split(',').map(tag => tag.trim());
      
      await axios.post(`${API_URL}/add`, {
        ...formData,
        tags: tagsArray
      });

      setMessage("Đã nạp kiến thức thành công!");
      // Reset form để nhập tiếp
      setFormData({ problem: '', solution: '', explanation: '', tags: '' });
      setTimeout(() => setMessage(''), 3000);
    } catch (error) {
      setMessage("Lỗi khi lưu: " + error.message);
    }
  };

  return (
    <div className="container">
      {/* THANH ĐIỀU HƯỚNG */}
      <nav className="navbar">
        <button 
          className={view === 'search' ? 'active' : ''} 
          onClick={() => setView('search')}
        >
          🔍 Tìm kiếm (Recall)
        </button>
        <button 
          className={view === 'add' ? 'active' : ''} 
          onClick={() => setView('add')}
        >
          ➕ Nạp kiến thức (Learn)
        </button>
      </nav>

      {/* VIEW TÌM KIẾM */}
      {view === 'search' && (
        <div className="search-view">
          <form onSubmit={handleSearch} className="search-box">
            <input
              type="text"
              placeholder="Bạn đang gặp vấn đề gì? (Ví dụ: lỗi pandas copy...)"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              autoFocus
            />
            <button type="submit">Tìm</button>
          </form>

          <div className="results-list">
            {results.map((item) => (
              <div key={item.id} className="note-card">
                <div className="note-header">
                  <h3>{item.metadata.problem}</h3>
                  <span className="score">{(item.score * 100).toFixed(0)}% relevant</span>
                </div>
                
                <div className="note-explanation">
                  <strong>💡 Tại sao:</strong> {item.metadata.explanation}
                </div>

                <div className="note-code">
                  <SyntaxHighlighter language="python" style={atomOneDark}>
                    {item.metadata.solution}
                  </SyntaxHighlighter>
                </div>
                
                <div className="note-tags">
                  {item.metadata.tags.split(',').map(tag => (
                    <span key={tag} className="tag">#{tag}</span>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* VIEW THÊM MỚI */}
      {view === 'add' && (
        <div className="add-view">
          <h2>Ghi chép kiến thức mới</h2>
          {message && <p className="status-msg">{message}</p>}
          
          <form onSubmit={handleAdd}>
            <div className="form-group">
              <label>1. Vấn đề (Triệu chứng)</label>
              <input 
                type="text" 
                placeholder="Ví dụ: Không thể convert string sang int"
                value={formData.problem}
                onChange={e => setFormData({...formData, problem: e.target.value})}
                required
              />
            </div>

            <div className="form-group">
              <label>2. Giải pháp (Code Snippet)</label>
              <textarea 
                rows="5"
                placeholder="Paste code vào đây..."
                value={formData.solution}
                onChange={e => setFormData({...formData, solution: e.target.value})}
                required
                className="code-input"
              />
            </div>

            <div className="form-group">
              <label>3. Giải thích (Kỹ thuật Feynman - BẮT BUỘC)</label>
              <textarea 
                rows="3"
                placeholder="Giải thích bằng ngôn ngữ của bạn: Tại sao code trên hoạt động?"
                value={formData.explanation}
                onChange={e => setFormData({...formData, explanation: e.target.value})}
                required
              />
            </div>

            <div className="form-group">
              <label>Tags (cách nhau dấu phẩy)</label>
              <input 
                type="text"
                placeholder="python, error, basics"
                value={formData.tags}
                onChange={e => setFormData({...formData, tags: e.target.value})}
              />
            </div>

            <button type="submit" className="save-btn">Lưu vào bộ não</button>
          </form>
        </div>
      )}
    </div>
  );
}

export default App;