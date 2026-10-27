import { useEffect, useState } from 'react'

interface Node {
  id: number
  is_exhausted: boolean
  quota: number
  error_429_count: number
}

// Shape of GET /api/state (see backend/src/main.rs StateResponse)
interface StateResponse {
  nodes: Node[]
  strategy: 'Random' | 'GlobalLeast' | 'P2C'
  total_requests: number
  total_429s: number
}

function App() {
  const [nodes, setNodes] = useState<Node[]>([])
  const [stats, setStats] = useState<Omit<StateResponse, 'nodes'> | null>(null)

  useEffect(() => {
    const fetchState = () => {
      fetch('http://localhost:3000/api/state')
        .then((res) => res.json())
        .then((data: StateResponse) => {
          setNodes(data.nodes)
          setStats({ strategy: data.strategy, total_requests: data.total_requests, total_429s: data.total_429s })
        })
        .catch((error) => console.error('Failed to fetch state:', error))
    }

    fetchState()
    const interval = setInterval(fetchState, 1000)
    return () => clearInterval(interval)
  }, [])

  return (
    <div className="min-h-screen bg-gray-900 flex flex-col items-center py-10">
      <h1 className="text-white text-4xl font-bold mb-4">P2C Simulator - V1</h1>
      {stats && (
        <p className="text-gray-300 font-mono text-sm mb-8">
          strategy={stats.strategy} · requests={stats.total_requests} · 429s={stats.total_429s} (
          {stats.total_requests > 0 ? ((100 * stats.total_429s) / stats.total_requests).toFixed(2) : '0.00'}%) ·
          exhausted={nodes.filter((n) => n.is_exhausted).length}/{nodes.length}
        </p>
      )}
      <div className="grid grid-cols-10 gap-3">
        {nodes.map((node) => (
          <div
            key={node.id}
            title={`Node ${node.id}`}
            className={`w-12 h-12 rounded flex items-center justify-center text-xs text-white font-medium transition-colors duration-300 ${
              node.is_exhausted ? 'bg-red-500' : 'bg-green-500'
            }`}
          >
            {node.id}
          </div>
        ))}
      </div>
    </div>
  )
}

export default App

