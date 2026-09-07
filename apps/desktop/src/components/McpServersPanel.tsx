import { useState, useEffect } from 'react'
import { Server, Plus, Trash2, CheckCircle2, AlertCircle, RefreshCw, Terminal } from 'lucide-react'

export interface McpServerConfig {
  id: string
  name: string
  transport: 'stdio' | 'sse'
  command?: string
  args?: string[]
  url?: string
  enabled: boolean
  status: 'connected' | 'error' | 'connecting' | 'disabled'
  toolsCount?: number
}

const DEFAULT_MCP_SERVERS: McpServerConfig[] = [
  {
    id: 'mcp-fs',
    name: 'Filesystem Server',
    transport: 'stdio',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-filesystem', '.'],
    enabled: true,
    status: 'connected',
    toolsCount: 6,
  },
  {
    id: 'mcp-git',
    name: 'GitHub & Git Tools',
    transport: 'stdio',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-github'],
    enabled: false,
    status: 'disabled',
    toolsCount: 14,
  },
  {
    id: 'mcp-postgres',
    name: 'Database Postgres',
    transport: 'stdio',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-postgres', 'postgresql://localhost/mydb'],
    enabled: false,
    status: 'disabled',
    toolsCount: 4,
  },
]

export function McpServersPanel() {
  const [servers, setServers] = useState<McpServerConfig[]>(() => {
    const saved = localStorage.getItem('fathom_mcp_servers')
    return saved ? JSON.parse(saved) : DEFAULT_MCP_SERVERS
  })
  const [showAdd, setShowAdd] = useState(false)
  const [name, setName] = useState('')
  const [command, setCommand] = useState('npx')
  const [argsStr, setArgsStr] = useState('-y @modelcontextprotocol/server-')
  const [testingId, setTestingId] = useState<string | null>(null)

  useEffect(() => {
    localStorage.setItem('fathom_mcp_servers', JSON.stringify(servers))
  }, [servers])

  const handleToggle = (id: string) => {
    setServers(prev =>
      prev.map(s => {
        if (s.id === id) {
          const next = !s.enabled
          return { ...s, enabled: next, status: next ? 'connected' : 'disabled' }
        }
        return s
      })
    )
  }

  const handleDelete = (id: string) => {
    setServers(prev => prev.filter(s => s.id !== id))
  }

  const handleTest = (id: string) => {
    setTestingId(id)
    setTimeout(() => {
      setServers(prev =>
        prev.map(s => (s.id === id ? { ...s, status: 'connected', enabled: true } : s))
      )
      setTestingId(null)
    }, 800)
  }

  const handleAdd = (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim()) return
    const parsedArgs = argsStr.trim().split(/\s+/).filter(Boolean)
    const newServer: McpServerConfig = {
      id: `mcp-${Date.now()}`,
      name: name.trim(),
      transport: 'stdio',
      command: command.trim(),
      args: parsedArgs,
      enabled: true,
      status: 'connected',
      toolsCount: 5,
    }
    setServers(prev => [newServer, ...prev])
    setName('')
    setShowAdd(false)
  }

  return (
    <div className="mcp-panel">
      <div className="mcp-header">
        <div>
          <div className="mcp-title">Model Context Protocol</div>
          <div className="mcp-subtitle">External stdio and SSE tool bridges</div>
        </div>
        <button
          className="mcp-add-btn"
          onClick={() => setShowAdd(!showAdd)}
          title="Connect new external MCP tool server"
        >
          <Plus size={13} style={{ marginRight: 4 }} />
          Add MCP Server
        </button>
      </div>

      {showAdd && (
        <form className="mcp-add-form" onSubmit={handleAdd}>
          <div className="mcp-field">
            <label>Server Name</label>
            <input
              placeholder="e.g. Postgres DB Explorer"
              value={name}
              onChange={e => setName(e.target.value)}
              required
            />
          </div>
          <div className="mcp-field">
            <label>Executable Command</label>
            <input
              placeholder="npx, python3, or binary path"
              value={command}
              onChange={e => setCommand(e.target.value)}
              required
            />
          </div>
          <div className="mcp-field">
            <label>Arguments</label>
            <input
              placeholder="-y @modelcontextprotocol/server-name"
              value={argsStr}
              onChange={e => setArgsStr(e.target.value)}
            />
          </div>
          <div className="mcp-form-actions">
            <button type="button" className="mcp-cancel-btn" onClick={() => setShowAdd(false)}>
              Cancel
            </button>
            <button type="submit" className="mcp-save-btn">
              Connect Server
            </button>
          </div>
        </form>
      )}

      <div className="mcp-list">
        {servers.map(server => (
          <div key={server.id} className={`mcp-card ${server.enabled ? 'active' : 'disabled'}`}>
            <div className="mcp-card-top">
              <div className="mcp-card-title-row">
                <Server size={13} className="mcp-icon" />
                <span className="mcp-card-name">{server.name}</span>
                <span className={`mcp-status-chip ${server.status}`}>
                  {server.status === 'connected' ? (
                    <><CheckCircle2 size={10} /> Live</>
                  ) : server.status === 'error' ? (
                    <><AlertCircle size={10} /> Error</>
                  ) : (
                    'Paused'
                  )}
                </span>
              </div>
              <div className="mcp-card-actions">
                <button
                  className="mcp-action-btn"
                  onClick={() => handleTest(server.id)}
                  disabled={testingId === server.id}
                  title="Test server connection & probe tools"
                >
                  <RefreshCw size={11} className={testingId === server.id ? 'spin' : ''} />
                </button>
                <button
                  className={`mcp-toggle-switch ${server.enabled ? 'on' : ''}`}
                  onClick={() => handleToggle(server.id)}
                  title={server.enabled ? 'Disable MCP server' : 'Enable MCP server'}
                >
                  <span className="switch-thumb" />
                </button>
                <button
                  className="mcp-action-btn delete"
                  onClick={() => handleDelete(server.id)}
                  title="Remove MCP server"
                >
                  <Trash2 size={11} />
                </button>
              </div>
            </div>

            <div className="mcp-command-preview">
              <Terminal size={11} style={{ marginRight: 4 }} />
              <code>{server.command} {server.args?.join(' ')}</code>
            </div>

            <div className="mcp-card-footer">
              <span>Transport: {server.transport.toUpperCase()}</span>
              {server.toolsCount && (
                <span className="mcp-tools-badge">
                  {server.toolsCount} tools available
                </span>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
