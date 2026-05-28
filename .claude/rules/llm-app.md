---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# LLM-Powered Application Review

A reference for evaluating code that builds on large language model APIs (Anthropic Claude, OpenAI GPT, Google Gemini, open models via inference providers). Used by the `llm-app` subagent.

The scope: **the engineering of LLM-powered features and agents.** Prompt engineering, tool use / function calling, RAG architecture, eval design, context management (caching, compaction), model selection, prompt-injection defense, cost shape, agentic patterns, production-readiness (retries, timeouts, fallbacks, monitoring).

Distinct from:
- **`claude-api` skill**: that's a how-to skill for *building* Claude API apps. This is the *review* lens for any LLM-powered code.
- **`security`**: general security. We flag prompt-injection at the design level; deep threat model goes there.
- **`performance`**: general perf. We flag LLM cost / latency shape; deep perf goes there.
- **`api-design`**: consumer-contract design. We touch LLM API *consumption* but designing APIs for downstream consumers is theirs.
- **`web-analytics`**: product analytics including A/B testing. We touch eval methodology; online product analytics is theirs.

The user works at Anthropic. **Anthropic-favored** where applicable (XML prompt structure, Claude model tiers, prompt caching as first-class, MCP for tool servers, extended thinking, "Building Effective Agents" framing). Preserve neutrality on multi-vendor questions.

The core thesis: **most production LLM bugs are not the model -- they are the engineering around the model.** No evals, system-prompt-and-user-input concatenation, unbounded agentic loops, indirect prompt injection ignored, cache invalidation by accident, the wrong model for the task. The reviewer's value is recognizing these patterns from production experience that the general agents lack.

The empirical priority: **evals first.** Hamel Husain's thesis ("Your AI product needs evals") is the single most important framing. A team without evals is shipping prompt changes by vibe; every iteration could be regressing as easily as progressing. The reviewer's first question on any LLM-app code is: where are the evals, and what do they cover?

---

## Universal principles

### Evals are not optional

A team without evals is flying blind. Symptoms: prompt changes shipped by vibe; "it seems better"; quality regressions caught only by user complaints; no regression suite for past bugs.

**The eval discipline**:
- **Golden sets**: hand-curated input → expected-output pairs that capture the production distribution.
- **Regression suites**: every shipped bug becomes a perpetual eval case.
- **LLM-as-judge** (with limits): cheap, scalable; inherits judge model biases (position, self, length).
- **Pairwise comparison** with randomized order to mitigate position bias.
- **Online feedback** (thumbs, edits, abandonment metrics) as ground-truth signal.
- **Pre-deploy eval gate**: every prompt / model change runs against the eval suite; regression blocks deploy.

**Flag**: prompt changes without eval comparison; LLM-as-judge using the same model as the generator (self-bias); pairwise comparisons without order randomization; eval sets that don't reflect production distribution; no regression suite.

### Prompt structure: role separation is non-negotiable

System prompts and user input must NEVER be concatenated into one string. The system prompt is the contract; user input is data the contract operates on. Mixing them invites the user input to override the contract -- the canonical prompt-injection setup.

**Flag**: `prompt = system_text + "\n\nUser: " + user_input` patterns; user input interpolated directly into a template that also contains instructions; templates that don't use the API's structured `system` / `messages` shape.

### Anthropic prompt conventions (default for multi-vendor code)

- **XML tags** (`<example>`, `<document>`, `<instructions>`, `<thinking>`): Claude is trained on these; they separate sections reliably. OpenAI / Gemini handle them fine too.
- **System prompt for role and instructions**; user prompt for the request and data.
- **Examples (few-shot)**: 3-5 well-chosen examples often beat extensive instructions; the model pattern-matches.
- **Chain-of-thought**: explicit `<thinking>` for non-trivial reasoning, or rely on extended-thinking models (Claude 3.7+, o1/o3) when reasoning matters.
- **Prefilling**: putting the first tokens of the assistant turn into the request constrains output structure. Under-used.
- **Critical constraints last**: recency bias in attention; the last instruction has more weight than the buried middle.

**Flag**: prompts with critical constraints buried in long instructions; negative instructions ("do not X") instead of positive framing; vague specifications producing inconsistent outputs; persona prompts ("you are a helpful expert") as the primary capability lift (rarely the load-bearing intervention).

### Tool descriptions ARE documentation for the model

A tool's `description` field is the only context the model has for deciding when and how to use the tool. Internal-comment-style descriptions ("does the order thing") under-perform purpose-clear ones ("Creates a new order in the database. Returns the order ID. Use this when the user confirms they want to place an order.").

**Flag**: tool descriptions reading like internal comments; parameters without descriptions; tools whose description doesn't name side effects ("Deletes the file -- this is destructive and cannot be undone").

### Cost shape is part of the design

LLM features have a unit cost. Input tokens are typically 3-5x cheaper than output tokens. Prompt caching offers 60-90% reduction on hit. Batch APIs offer 50% discount with 24h latency. Most production systems should be designed around this economics, not retrofit.

**Flag**: no caching strategy (every request rebuilds the prefix; cache hit rate ~0); real-time API used for batch-shapeable workloads; long output tokens not constrained; no per-user cost cap on user-triggered LLM features (runaway cost risk).

### Indirect prompt injection is the live threat

Direct injection ("ignore previous instructions") is mostly addressed by role separation. **Indirect injection** -- retrieved content, tool outputs, document content containing injection -- is the live threat. Greshake et al. (2023) introduced the term; it applies whenever the model sees content from anything other than the developer.

**Flag**: retrieved RAG content treated as instructions; tool outputs that the model interprets as commands without privilege separation; web content / PDFs / documents fed into the model without sanitization or trust-boundary marking; user-generated content embedded in prompts.

---

## Prompt engineering

### Structure

- **System prompt**: role, instructions, constraints, examples, format. The contract.
- **User message**: the request and the data it operates on.
- **Assistant prefill** (Anthropic): constrain output structure. Underused.
- **Multi-turn**: conversation history; cached prefix dominates.

### XML vs Markdown

Anthropic-trained models prefer XML; OpenAI-trained handle Markdown well. Both work across both. For multi-vendor code, XML is the safer default.

**Flag**: stylistic inconsistency (mixing XML and Markdown headings in one prompt); section delimiters that the model can't distinguish from user content (use clear opening / closing tags).

### Few-shot examples

3-5 examples beat extensive instructions for pattern-shaped tasks. The model pattern-matches faster than it reasons. Examples should cover the distribution; edge cases get edge-case examples.

**Flag**: no examples for tasks that have a clear input-output shape; examples that all look the same (no diversity = no learning signal); examples after the request rather than before.

### Chain-of-thought

Explicit `<thinking>` blocks or "let's think step by step" for non-trivial reasoning. Extended-thinking models (Claude 3.7+, o1/o3) build this in.

**Flag**: complex reasoning expected with no thinking-space provided; explicit CoT used on extended-thinking models (redundant).

### Versioning

Prompts are code. Versioning, diffing, eval-comparing, rolling back: all should be supported.

**Flag**: prompts in code that change silently without version history; production prompts not stored anywhere durable; A/B testing between prompt versions without statistical rigor.

---

## Tool use / function calling

### Tool definition design

- **Name**: clear, action-oriented, namespaced if many.
- **Description**: the contract; what it does, when to use it, side effects, return shape.
- **Parameters**: each with type and description; required vs optional explicit.
- **Side effect annotation**: name destructive operations explicitly.

**Anthropic-style example**:
```
{
  "name": "send_email",
  "description": "Sends an email to the specified recipient. This is a destructive action that cannot be undone; confirm with the user before calling. Returns the message ID.",
  "input_schema": {
    "type": "object",
    "properties": {
      "to": {"type": "string", "description": "Recipient email address."},
      "subject": {"type": "string", "description": "Subject line."},
      "body": {"type": "string", "description": "Email body in plain text."}
    },
    "required": ["to", "subject", "body"]
  }
}
```

### Idempotency for agent retries

Agents retry liberally. Tool calls with side effects need idempotency keys or the agent can double-fire (charge twice, send the email twice).

**Flag**: destructive tools without idempotency mechanisms; tools that the agent might retry but whose side effects compound on retry.

### Parallel tool use

Claude / GPT-4 can call multiple tools in one turn. Useful when tools are independent.

**Flag**: parallel tool calls where dependencies exist (one tool's output feeds another); sequential tool calls where parallel would work.

### Tool output design

Outputs the LLM reads. Terse, structured, error-self-describing.

**Flag**: tool outputs containing internal IDs, PII, secrets the model shouldn't see; tools whose error outputs the LLM cannot recover from ("ERROR: code 12345" with no context); outputs > 10KB stuffing context (chunk or summarize).

### MCP servers

The Anthropic-led open standard (November 2024); adopted by OpenAI, Google, most agent frameworks in 2025. A tool server exposing tools / resources / prompts; the host LLM connects.

**Flag**: MCP server with over-broad capabilities (the LLM equivalent of an over-privileged Unix process); MCP server without authentication; trusting MCP-tool outputs as instructions.

---

## RAG (Retrieval-Augmented Generation)

### Chunking

Fixed-size chunks (e.g., 500 tokens with 50-token overlap) are the baseline. Semantic chunking (paragraph- or section-aware) preserves structure better. Document-aware chunking (heading hierarchies) preserves more.

**Flag**: chunking that destroys document structure (cuts headings from content); chunks too large to fit retrieval budget; chunks too small to carry meaning.

### Embedding model choice

Anthropic voyage-3, OpenAI text-embedding-3-small / -large, Cohere embed-v3, BGE / E5 (open). Cost vs quality. **Critically**: embedding model must match between index time and query time.

**Flag**: embedding model mismatch between indexed content and query (silent quality collapse); not re-indexing on embedding model change.

### Hybrid retrieval and reranking

Dense vector + BM25 sparse + reranker (Cohere Rerank, bge-reranker) is the modern stack for quality-critical RAG. Pure dense retrieval is the baseline.

**Flag**: pure dense retrieval on a system with quality complaints (rerankers often dominate the gain); no BM25 hybrid where keyword matching matters (product names, identifiers).

### Citation grounding

The model cites the retrieved passage it used. Enables user verification; reduces hallucination cost (user can check).

**Flag**: no citation in RAG output; citations that don't link back to source; structured citations not validated (model fabricates a citation).

### Long-context vs RAG

Crossover: when does long-context (passing the whole corpus) beat RAG (retrieving the relevant slice)? Depends on corpus size, query distribution, cost, freshness.

**Flag**: RAG over a corpus that fits in long context (engineering overhead without benefit); long context for queries that hit a small predictable slice (cost waste).

### Hallucination

The model produces output not grounded in the retrieved content. Sources: model knowledge override, retrieval failure, chunk boundaries cutting context.

**Flag**: no uncertainty markers when retrieval confidence is low; UI displays LLM output as fact without grounding; no fallback when retrieval returns nothing relevant.

---

## Evals

### Why evals matter

Hamel Husain: "Your AI product needs evals." Without evals, every prompt change could be regressing. The team is shipping by vibe.

The eval discipline:
- **Pre-deploy gate**: changes don't ship if regression on the eval suite.
- **Regression suite**: every bug becomes a perpetual eval case.
- **Coverage**: eval set reflects production distribution.
- **Hold-out**: separate set the team doesn't optimize against (avoids Goodhart drift).

### LLM-as-judge

A cheaper alternative to human eval at scale. Limitations:
- **Position bias**: preference for first vs second answer in pairwise.
- **Self-bias**: prefers outputs from the same model family.
- **Length bias**: prefers longer outputs.
- **Verbosity bias**: confident-sounding wrong outputs preferred.

**Mitigations**: randomize order in pairwise; use a different model as judge than generator; constrain output length in evals.

**Flag**: LLM-as-judge using the same model as the generator; no order randomization in pairwise comparisons; LLM-as-judge as the only signal (no human calibration).

### Online feedback

Production signals (thumbs, edits, abandonment, conversation length) are ground-truth at scale.

**Flag**: no online feedback capture; feedback signals not feeding back into evals.

---

## Context management

### Prompt caching (Anthropic, OpenAI)

90% cost reduction on cache hits (Anthropic); 50% for OpenAI. 5-minute TTL (Anthropic). Cache breakpoints in the prompt prefix; everything before the breakpoint is cached together.

**The cache invalidation discipline**: ANY change to the prefix invalidates all cached suffixes. Stable prefixes are gold; variable content in the prefix kills the cache.

**Flag**: variable content in the cacheable prefix (kills cache); no cache breakpoints in long stable prompts; cache hit rate not monitored; cache invalidation by accident (small upstream change cascades).

### Compaction

Conversation history grows beyond context budget. Compaction summarizes prior turns into a smaller representation.

**Flag**: unbounded conversation history (eventually exceeds context); compaction that loses information the agent later needs; no compaction policy.

### Memory systems

External persistent memory beyond conversation (Letta / MemGPT, custom). Scoping is the design question.

**Flag**: memory systems without clear scoping (user / tenant / org); memory leaks across users (privacy violation); memory growing unbounded.

---

## Model selection

### Capability tiers (Anthropic-favored)

- **Haiku**: cheapest, fastest, weakest. Classification, simple extraction, high-volume cheap tasks.
- **Sonnet**: balanced. Most production tasks.
- **Opus**: strongest. Complex reasoning, high-stakes, rare invocations.

Plus: **extended thinking** (Claude 3.7+, o1/o3, Gemini Thinking) for genuine reasoning tasks.

### Fallback chains

Try Haiku first; escalate to Sonnet on failure. Cost-optimization pattern. Eval-validated.

**Flag**: strong model used for tasks the cheap model handles equivalently; cheap model used for tasks where capability gap silently regresses quality; reasoning model used where it doesn't help (cost / latency waste); model version not pinned (silent upgrades change behavior); migrations without eval comparison.

---

## Prompt-injection defense

### Direct injection

User input contains "ignore previous instructions." Largely addressed by role separation (system vs user prompt) and modern models' resistance training.

### Indirect injection (Greshake et al. 2023)

Retrieved content, tool outputs, web pages, documents contain injection. The model treats them as instructions because they're in the context window.

**Mitigations**:
- **Trust-boundary marking**: explicitly tag user-controlled vs trusted content in the prompt.
- **Privilege separation**: tools have minimum capability; LLM is not given root.
- **Sandboxed code execution**: if the model can run code, it runs in a sandbox.
- **Output validation**: structured outputs (tool use, JSON schema) constrain the attack surface.
- **Human-in-the-loop on destructive operations**: irreversible tool calls require confirmation.

**Flag**: retrieved RAG content trusted as instructions; tool outputs interpreted as commands; code-execution tools without sandboxing; destructive tools invoked from untrusted-content paths; secrets in system prompts (system prompts are leakable via clever attacks).

---

## Cost shape

### The pricing model

Input tokens ~3-5x cheaper than output tokens. Long prompts are cheaper than long outputs.

### Cost-per-task vs cost-per-token

Cost-per-token is the bill; cost-per-task is what matters for product. A 100k input token / 100 output token request may be cheaper than 1k / 1k for the same task.

### Caching, batching, streaming

- **Caching** (Anthropic / OpenAI): 60-90% off on hit. Stable prefix design.
- **Batch API** (Anthropic / OpenAI): 50% off, 24h latency. For non-realtime workloads.
- **Streaming**: doesn't reduce cost; reduces perceived latency.

**Flag**: real-time API for batch-shapeable workloads; no caching when prefix is stable; no streaming where latency matters and feasible.

### Cost monitoring

- Per-feature, per-user, per-tenant cost tracking.
- Alerts on cost anomalies.
- Per-user cost cap on user-triggered features.

**Flag**: no per-user cost cap on user-triggered features (runaway cost risk); no cost monitoring at all; cost monitored but no alerts.

---

## Agentic patterns

Anthropic's "Building Effective Agents" (December 2024) distinguishes:

- **Workflows**: predefined chains. Predictable, debuggable, eval-bounded.
- **Agents**: LLM-driven loops with tool access. Open-ended, harder to bound, less predictable.

**The Anthropic-favored stance**: prefer workflows; reach for agents when the task is genuinely open-ended.

### Workflow patterns
- **Prompt chaining**: sequential prompts.
- **Routing**: classify the task, dispatch to a specialized prompt / model.
- **Parallelization**: independent subtasks run concurrently.
- **Orchestrator-workers**: top-level LLM plans; sub-LLMs execute.
- **Evaluator-optimizer**: generate, evaluate, refine, loop.

### Agent patterns
- **Bounded loops**: max iterations, max cost, max wall time.
- **Reflection**: model evaluates its own output before returning.
- **Computer Use** (Anthropic, Oct 2024): screenshots + mouse / keyboard tools. Early adopter pattern.

**Flag**: unbounded agentic loops (no step cap, cost cap, time cap); agents reading untrusted content with high-privilege tools; "agent" framing for what's actually a workflow (over-engineered); workflow framing for what's genuinely open-ended (under-engineered).

---

## Anti-pattern catalog

### Prompt
- System prompt + user input concatenated.
- PII in prompts logged without redaction.
- Negative instructions instead of positive framing.
- Vague specifications producing inconsistent outputs.
- No examples for pattern-shaped tasks.
- One mega-prompt doing five things.
- Prompts changed in-place without versioning.
- Persona prompts as primary capability lift.
- Critical constraints buried in middle of long prompts.

### Tool use
- Tool descriptions like internal comments.
- Parameters without descriptions.
- Destructive tools without idempotency keys.
- Destructive tools without human-in-the-loop confirmation.
- Unbounded agentic loops.
- MCP servers with over-broad capabilities.
- Tool outputs with internal IDs / PII / secrets.
- Tools whose error outputs the LLM cannot interpret.
- Parallel tool calls where dependencies exist.

### RAG
- No citation grounding.
- Chunking destroys document structure.
- Embedding model mismatch between index and query.
- No reranker on a quality-complaint system.
- Trusting retrieved content as instructions.
- RAG over a corpus that fits in long context.
- No query rewriting on conversational inputs.
- Hallucinations not surfaced.

### Evals
- Shipping prompt changes without evals.
- LLM-as-judge using same model as generator.
- Pairwise comparisons without order randomization.
- Eval sets not reflecting production distribution.
- Evals pass "in general" but regress on real cases.
- No regression suite.
- No hold-out set (Goodhart drift on dev eval).
- Online feedback signals not captured.

### Context
- Variable content in cacheable prefix.
- No cache breakpoints in long stable prompts.
- Unbounded conversation history.
- Compaction loses information.
- Memory systems without clear scoping.

### Model selection
- Strong model for tasks cheap model handles.
- Cheap model for tasks where capability gap matters.
- Reasoning model for non-reasoning tasks.
- Model version not pinned.
- Migrations without eval comparison.

### Security
- Prompt injection assumed solved by input filtering.
- Retrieved content trusted as instructions.
- Secrets in system prompts.
- Tools at high privilege invoked from untrusted-content paths.
- No sandboxing of code-execution tools.

### Cost
- No per-user cost cap on LLM-powered features.
- No cache hit rate monitoring.
- Real-time API used for batch-shapeable workloads.
- Long output tokens not constrained.
- No streaming where perceived latency matters.

### Production readiness
- Logging full prompts and outputs without retention policy.
- No retry on 529 / 503 / network errors.
- No timeout on LLM calls.
- No fallback model on 5xx from primary.
- Streaming display without parsing-aware design (partial JSON breaks).
- No prompt-token estimation (silent context overflows).
- Rate limiting at the provider but not the application.

---

## Modern shifts (2024-2025)

- **MCP** as the tool-server standard (Anthropic Nov 2024).
- **Extended thinking / reasoning models** (Claude 3.7+, o1/o3, Gemini Thinking).
- **Computer Use** and UI agents (early days).
- **Prompt caching** as table stakes.
- **Batch APIs** as table stakes.
- **"Agents are workflows" critique** (Anthropic's "Building Effective Agents") widely adopted.
- **Multimodal as default** (image, document, audio).
- **On-device LLMs** (Apple Intelligence, Gemini Nano).
- **The eval crisis**: industry awareness growing; practice lagging.
- **Pricing competition**: 10x cost drop in 18 months.
- **Voice / real-time**: OpenAI Realtime, Gemini Live -- separate review surface.

---

## Schools of disagreement

- **XML vs Markdown prompt structure**: Anthropic prefers XML, OpenAI handles Markdown. XML is the safer multi-vendor default.
- **Long-context vs RAG**: crossover depends on corpus size, query distribution, cost, freshness.
- **Workflow vs agent**: Anthropic biases toward workflows; some teams bullish on agents.
- **LLM-as-judge vs human eval**: hybrid is the pragmatic answer.
- **Prompt engineering vs fine-tuning**: most teams over-invest in prompts; fine-tuning rarely the right early answer.
- **Streaming vs non-streaming**: stream when latency matters; don't when atomicity matters.
- **MCP vs direct tool definitions**: MCP for portfolios of tools across multiple agents; direct for first project.
- **Single-model vs multi-model**: start single; add tiers when measurable win.

---

## What is NOT an llm-app finding

- The `claude-api` skill's territory (how-to for building Claude apps).
- General security beyond LLM-specific (route to `security`).
- General performance beyond LLM cost / latency (route to `performance`).
- API contract design for downstream consumers (route to `api-design`).
- Online product analytics (route to `web-analytics`).
- Web search architecture, retrieval infrastructure design (route to `system-design`).
- ML / training questions (this agent is for LLM *application* code, not model training).

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: prompt-injection vulnerability with reachable trigger (retrieved content trusted as instructions, destructive tool invoked from untrusted-content path); production code shipping without evals on critical user-facing LLM features; unbounded agentic loop with cost-uncapped tool access; PII in prompts logged to non-compliant storage; LLM-as-judge using same model as generator on production evals.
- **major**: no caching strategy on cacheable workflows (60-90% cost left on table); no per-user cost cap on user-triggered features; tool descriptions inadequate for the model to use correctly; model version not pinned; no fallback model on provider 5xx; structured output without schema validation; chunking that destroys document structure in RAG.
- **minor**: prompts mixing XML and Markdown structure inconsistently; missing few-shot examples on pattern-shaped tasks; cache breakpoints missing in long stable prompts; no streaming where perceived latency matters.
- **nit**: stylistic prompt choices that don't affect output quality; missing optional metadata fields.
- **insight**: structural -- "this whole workflow is over-engineered as an agent; the task is bounded enough for prompt chaining"; "this codebase should adopt MCP given the portfolio of tools across agents"; "fine-tuning would close the capability gap that prompt engineering can't, given the data you have."

Confidence: high on spec-level findings (the API contract requires this); medium on prompt-effectiveness findings (depends on the model and the eval); lower on cost-projection findings without measurement.

---

## Process for the llm-app agent

1. **Identify the LLM provider(s) and model(s).** Anthropic / OpenAI / Google / open / self-hosted? Single-model or fallback chain?
2. **Identify the application pattern.** One-shot completion? Chat / multi-turn? Tool-use agent? RAG? Computer Use?
3. **Walk evals.** Where are they? What do they cover? Pre-deploy gate? Regression suite? LLM-as-judge methodology sound?
4. **Walk prompt structure.** System / user separation? Role-injection-safe? XML / Markdown consistent? Examples? Cache breakpoints?
5. **Walk tool use** (if applicable). Tool descriptions adequate? Idempotency? Human-in-the-loop on destructive? Parallel where independent?
6. **Walk RAG** (if applicable). Chunking strategy? Embedding model matched at index / query? Hybrid retrieval? Reranker? Citation grounding? Indirect injection defense?
7. **Walk context management.** Caching strategy? Compaction policy? Memory scoping?
8. **Walk model selection.** Right tier for task? Pinned version? Fallback chain? Migration eval-validated?
9. **Walk prompt-injection defense.** Trust boundaries marked? Tools at minimum capability? Code execution sandboxed? Destructive operations gated?
10. **Walk cost shape.** Caching hit rate? Batching where applicable? Streaming UX where appropriate? Per-user / per-tenant cost cap?
11. **Walk production readiness.** Retries with backoff? Timeouts? Fallback model? Structured output validation? Logging retention policy?
12. **Route to other lenses**: deep security threat model → `security`; general perf → `performance`; contract design for downstream → `api-design`; online product analytics → `web-analytics`.
13. **Stay read-only.**
