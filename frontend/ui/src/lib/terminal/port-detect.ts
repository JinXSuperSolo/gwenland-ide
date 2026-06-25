const URL_PATTERN = /(https?:\/\/(?:localhost|127\.0\.0\.1|0\.0\.0\.0):(\d+))/i

const PORT_PATTERNS = [
  /Local:\s+https?:\/\/(?:localhost|127\.0\.0\.1|0\.0\.0\.0):(\d+)/i,
  /listening on.*:\s*(\d+)/i,
  /ready on.*:\s*(\d+)/i,
  /started server on.*:\s*(\d+)/i,
  /running at.*:\s*(\d+)/i,
  /:(\d+)\s*$/m,
]

function validPort(value: string): number | null {
  const port = Number(value)
  return Number.isInteger(port) && port >= 1 && port <= 65535 ? port : null
}

function normalizeUrl(url: string): string {
  return url.replace('0.0.0.0', 'localhost')
}

function stripAnsi(output: string): string {
  return output.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, '')
}

export function detectPort(output: string): number | null {
  const clean = stripAnsi(output)
  const urlMatch = clean.match(URL_PATTERN)
  if (urlMatch) return validPort(urlMatch[2])

  for (const pattern of PORT_PATTERNS) {
    const match = clean.match(pattern)
    if (match) {
      const port = validPort(match[1])
      if (port) return port
    }
  }
  return null
}

export function detectPreviewTarget(output: string): { port: number; url: string } | null {
  const clean = stripAnsi(output)
  const urlMatch = clean.match(URL_PATTERN)
  if (urlMatch) {
    const port = validPort(urlMatch[2])
    return port ? { port, url: normalizeUrl(urlMatch[1]) } : null
  }
  const port = detectPort(output)
  return port ? { port, url: `http://localhost:${port}` } : null
}
