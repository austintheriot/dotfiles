import {parseArgs} from './args'
import {execute} from './commands'
import {EXIT} from './config'
import {doctor, printChecks} from './doctor'
import {bunTooOld, MIN_BUN} from './doctor-core'
import {loadProfile} from './profile'
import {VmError} from './proc'

async function main(argv: string[], env: Record<string, string | undefined>): Promise<number> {
  if (bunTooOld(Bun.version, MIN_BUN)) {
    console.error(`vm needs Bun ${MIN_BUN} or later, and this is ${Bun.version}. Run: bun upgrade`)
    return EXIT.unavailable
  }
  const parsed = await parseArgs(argv, env)
  if (parsed.kind === 'exit') {
    process.stdout.write(parsed.stdout)
    process.stderr.write(parsed.stderr)
    return parsed.code
  }
  if (parsed.command.kind === 'doctor') return printChecks(doctor(env))
  try {
    return await execute(parsed.command, env, () => loadProfile(env).profile)
  } catch (error) {
    if (error instanceof VmError) {
      if (error.message !== '') console.error(error.message)
      return error.exitCode
    }
    throw error
  }
}

process.exit(await main(process.argv.slice(2), process.env))
