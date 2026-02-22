import { useEffect, useState } from 'react'

interface Node {
  id: number
  is_exhausted: boolean
}

function App() {
  const [nodes, setNodes] = useState<Node[]>([])

  useEffect(() => {
    const fetchState = () => {
      fetch('http://localhost:3000/api/state')
        .then((res) => res.json())
        .then((data: Node[]) => setNodes(data))
        .catch(() => {})
    }

    fetchState()
    const interval = setInterval(fetchState, 1000)
    return () => clearInterval(interval)
  }, [])

  return (
    <div className="min-h-screen bg-gray-900 flex flex-col items-center py-10">
      <h1 className="text-white text-4xl font-bold mb-10">P2C Simulator - V1</h1>
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

