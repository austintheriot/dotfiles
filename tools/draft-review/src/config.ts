import {existsSync, readFileSync} from 'node:fs'
import {homedir} from 'node:os'
import {join} from 'node:path'
import {z} from 'zod'
import type {Repo} from './args'
import {DraftReviewError, EXIT} from './exit'

export const DEFAULT_CONFIG_PATH = join(homedir(), '.claude', 'local', 'draft-review.json')
export const DEFAULT_STORE_ROOT = join(homedir(), '.cache', 'draft-review')

const RepositoryConfigSchema = z
  .object({uploadEndpoint: z.string().url().optional(), repositoryId: z.number().int().positive().optional(), attribution: z.string().optional()})
  .strict()

const ConfigSchema = z.object({repositories: z.record(z.string(), RepositoryConfigSchema)}).strict()

export type RepositoryConfig = z.infer<typeof RepositoryConfigSchema>
export type Config = z.infer<typeof ConfigSchema>

const describeIssue = (error: z.ZodError) => {
  const [issue] = error.issues
  if (issue === undefined) return 'invalid'
  return `${issue.path.length === 0 ? '(top level)' : issue.path.join('.')}: ${issue.message}`
}

function readJson(path: string): unknown {
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch (error) {
    throw new DraftReviewError(`cannot read the config file ${path}: ${error instanceof Error ? error.message : String(error)}`, EXIT.config)
  }
}

export function loadConfig(path: string): Config | undefined {
  if (!existsSync(path)) return undefined
  const parsed = ConfigSchema.safeParse(readJson(path))
  if (!parsed.success) throw new DraftReviewError(`the config file ${path} is invalid at ${describeIssue(parsed.error)}`, EXIT.config)
  return parsed.data
}

export function requireConfig(path: string): Config {
  const config = loadConfig(path)
  if (config === undefined) throw new DraftReviewError(`no config file at ${path}. Create it with an entry under "repositories" for this repository.`, EXIT.config)
  return config
}

export function repositoryConfig(config: Config, path: string, repo: Repo): RepositoryConfig {
  const entry = config.repositories[`${repo.owner}/${repo.name}`]
  if (entry === undefined) throw new DraftReviewError(`the config file ${path} has no entry for ${repo.owner}/${repo.name} under "repositories"`, EXIT.config)
  return entry
}
