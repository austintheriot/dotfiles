const CHROME_MAIN = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'

export function strayChromePids(psOutput: string, qaProfileDir: string): number[] {
  const profileFlag = `--user-data-dir=${qaProfileDir}`
  return psOutput.split('\n').flatMap(line => {
    const match = line.match(/^\s*(\d+)\s+(.*)$/)
    if (!match) return []
    const [, pid = '', command = ''] = match
    const words = command.slice(CHROME_MAIN.length).split(/\s+/).filter(Boolean)
    const isMain = command === CHROME_MAIN || command.startsWith(`${CHROME_MAIN} `)
    return isMain && !words.includes(profileFlag) ? [Number(pid)] : []
  })
}
