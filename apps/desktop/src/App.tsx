import { useState } from "react";

function App() {
  const [count, setCount] = useState(0);

  return (
    <div className="min-h-screen bg-gray-950 text-gray-100 flex flex-col items-center justify-center">
      <h1 className="text-4xl font-bold mb-4">ADE Desktop</h1>
      <p className="text-gray-400 mb-8">Agentic Development Environment</p>
      <button
        className="px-4 py-2 bg-blue-600 rounded hover:bg-blue-700 transition-colors"
        onClick={() => setCount((c) => c + 1)}
      >
        Count: {count}
      </button>
    </div>
  );
}

export default App;
