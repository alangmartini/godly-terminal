# Token Analyze

Analyze the current conversation to identify token waste patterns and suggest concrete improvements that reduce token usage while maintaining output quality.

## Usage

```
/token-analyze
```

## Instructions

Run through all 4 phases below, in order.

### Phase 1: Measure the Conversation

Scan the full conversation history. For each turn (user + assistant), estimate token usage by category:

| Category | What to Count |
|----------|---------------|
| **Tool output** | File reads, grep results, glob results, bash output |
| **Redundant reads** | Files read more than once, or read then re-read after minor edits |
| **Broad searches** | Glob/grep that returned many results when a targeted query would suffice |
| **Verbose output** | Bash commands with unnecessary flags (`--verbose`, `--nocapture`), large diffs dumped to output |
| **Agent overhead** | Agents spawned for tasks that could have been a single Glob/Grep/Read |
| **Wasted exploration** | Dead-end searches, reading files that weren't relevant |
| **Over-explanation** | Assistant text that restated what the user said, unnecessary preamble, or verbose reasoning |
| **Duplicate work** | Same search/read done both by main context and a sub-agent, or repeated across turns |

Build a table of the top waste sources, sorted by estimated token impact (high/medium/low).

### Phase 2: Identify Patterns

Group findings into actionable anti-patterns. Common ones:

| Anti-Pattern | Description | Fix |
|-------------|-------------|-----|
| **Shotgun grep** | Multiple broad greps instead of one targeted search | Use specific patterns, file type filters, `head_limit` |
| **Read-everything** | Reading entire large files when only a section was needed | Use `offset`/`limit` params, or grep first to find the line |
| **Agent for simple tasks** | Spawning an Explore agent for a single known-path file read | Use Read/Glob/Grep directly |
| **Echo-back** | Restating the user's request before acting on it | Go straight to the action |
| **Explain-the-obvious** | Long explanations of what a tool call will do | Just do it, explain only if non-obvious |
| **Defensive reading** | Reading files "just in case" that turned out irrelevant | Plan first, read only what's needed |
| **Full-file re-read** | Re-reading an entire file after a small Edit | Trust the edit, or read only the changed section |
| **Unbounded results** | Grep/glob without `head_limit` returning 50+ matches | Always set `head_limit` for exploratory searches |
| **Serial when parallel** | Making tool calls one at a time when they could be batched | Batch independent reads/greps in one turn |
| **Context-stuffing agent** | Agent that reads 10+ files and returns a massive summary | Scope agent tasks narrowly, ask specific questions |

For each anti-pattern found, cite the specific conversation turn where it occurred.

### Phase 3: Estimate Savings

For each anti-pattern, estimate the token savings if the optimal approach had been used:

```
Token Savings Estimate
======================
┌────────────────────────┬──────────┬─────────────┬──────────────────────────┐
│ Anti-Pattern           │ Occur.   │ Est. Waste  │ Suggested Fix            │
├────────────────────────┼──────────┼─────────────┼──────────────────────────┤
│ Shotgun grep           │ 3x       │ ~8K tokens  │ Use type filter + limit  │
│ Read-everything        │ 2x       │ ~6K tokens  │ offset/limit or grep     │
│ Echo-back              │ 5x       │ ~1K tokens  │ Skip restatement         │
│ Agent for simple task  │ 1x       │ ~4K tokens  │ Direct Glob call         │
├────────────────────────┼──────────┼─────────────┼──────────────────────────┤
│ TOTAL ESTIMATED WASTE  │          │ ~19K tokens │                          │
│ % of conversation      │          │ ~15%        │                          │
└────────────────────────┴──────────┴─────────────┴──────────────────────────┘
```

Use rough estimates — precision isn't needed, magnitude matters.

### Phase 4: Actionable Recommendations

Output a prioritized list of **concrete, specific** recommendations. Each recommendation should be something that can be applied in future conversations immediately.

Format:

```
Token Efficiency Recommendations (ordered by impact)
=====================================================

1. HIGH: [Title]
   Problem: [What happened in this conversation]
   Fix: [Exact change to behavior]
   Example: [Before → After]

2. MEDIUM: [Title]
   ...
```

Rules for recommendations:
- Maximum 7 recommendations. Focus on the highest-impact ones.
- Every recommendation must reference a specific moment in this conversation.
- "Fix" must be concrete enough to follow mechanically, not vague advice.
- Never recommend reducing quality, skipping verification, or cutting corners on correctness.
- If the conversation was already efficient, say so — don't manufacture fake findings.

### Output

End with a one-line summary:

```
Efficiency score: [X/10] — Estimated [N]K tokens saveable out of ~[M]K total ([P]%)
```

Where 10/10 means near-optimal token usage for the task complexity.
