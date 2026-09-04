# SOC Assessment Persistence

Local runtime support now exposes `SocCaseStore::persist_assessment`.

The operation starts one PostgreSQL transaction, inserts an immutable row into `soc_assessments`, updates the corresponding `soc_cases` projection (`risk_score`, `risk_level`, `ai_confidence`, `ai_verdict`, versions), requires exactly one case row, and commits. Any insert, projection or commit failure rolls back both operations. L2 can retain L1 through a separate row and `supersedes_assessment_id`; it never updates the prior assessment.

AI and analyst verdict domains remain separate. This method does not write analyst feedback, emit 90/90 notifications, invoke voice, or request Tier 2.

Compilation, formatting, unit tests and Clippy pass. Migration tables and constraints were validated in PostgreSQL 15 NONPROD. Runtime PostgreSQL integration is **UNVERIFIED** because CT134 intentionally has no Rust toolchain and no Core binary was deployed there; L1/L2 retention and projection rollback remain required.
