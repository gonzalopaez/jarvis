# SOC Assessment Persistence

Local runtime support now exposes `SocCaseStore::persist_assessment`.

The operation starts one PostgreSQL transaction, inserts an immutable row into `soc_assessments`, updates the corresponding `soc_cases` projection (`risk_score`, `risk_level`, `ai_confidence`, `ai_verdict`, versions), requires exactly one case row, and commits. Any insert, projection or commit failure rolls back both operations. L2 can retain L1 through a separate row and `supersedes_assessment_id`; it never updates the prior assessment.

AI and analyst verdict domains remain separate. This method does not write analyst feedback, emit 90/90 notifications, invoke voice, or request Tier 2.

The guarded integration harness ran this path against PostgreSQL 15.19 in temporary CT134. L1 (`81/63 SUSPICIOUS`) and L2 (`94/93 TRUE_POSITIVE`) produced two immutable rows; the case projection reflected L2 while L1 remained intact. Analyst `FALSE_POSITIVE` feedback was stored separately with AI snapshots. Both an injected post-insert failure and an FK insert failure rolled back completely, leaving no assessment or projection change. No production Core binary or database was used.
