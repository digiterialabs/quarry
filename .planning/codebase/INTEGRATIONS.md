# External Integrations

**Analysis Date:** 2026-02-24

## APIs & External Services

**Data Storage & Compute:**
- Apache DataFusion - Distributed SQL execution engine
  - SDK/Client: DataFusion Rust crate
  - Purpose: In-process query execution against structured data

- Apache Iceberg - Table format and metadata management
  - SDK/Client: Iceberg Rust crate
  - Purpose: S3-native table format with schema evolution and ACID guarantees

## Data Storage

**Databases:**
- S3 / S3-Compatible Object Storage
  - Connection: Environment configuration (AWS credentials or compatible)
  - Client: AWS SDK for Rust or compatible S3 client
  - Format: Iceberg tables containing Parquet/ORC data

**File Storage:**
- S3 (primary data lake)
- Local filesystem - Possible ephemeral caching during query execution

**Caching:**
- In-memory DataFusion execution context (per query, not persistent)
- No external caching layer currently planned

## Authentication & Identity

**Auth Provider:**
- External (AWS IAM or S3-compatible credentials)
  - Implementation: Standard S3 authentication via SDK
  - Configuration: Environment variables or credential files

**Semantic Layer Authentication:**
- Custom auth model (to be implemented)
  - Purpose: Control access to metrics, dimensions, and entities in semantic layer

## Monitoring & Observability

**Error Tracking:**
- Not detected - To be implemented as needed

**Logs:**
- Standard Rust logging (tracing/log crate likely)
- Query execution telemetry (to be designed)
- DataFusion query plans and statistics

## CI/CD & Deployment

**Hosting:**
- Not determined yet - Likely standalone binary deployment or container

**CI Pipeline:**
- Not detected - To be configured with GitHub Actions or similar

**Build Artifacts:**
- Native Rust binary (platform-specific)
- Potential Docker containerization

## Environment Configuration

**Required env vars (planned):**
- `AWS_ACCESS_KEY_ID` - S3 authentication
- `AWS_SECRET_ACCESS_KEY` - S3 authentication
- `AWS_REGION` - S3 region (if AWS)
- `S3_ENDPOINT` - Custom S3 endpoint (for non-AWS S3-compatible storage)
- `S3_BUCKET` - Data lake bucket name
- `ICEBERG_WAREHOUSE` - Iceberg metadata warehouse path

**Secrets location:**
- Environment variables or credential files (standard approach)
- Secrets manager integration (to be implemented)

## Webhooks & Callbacks

**Incoming:**
- Not planned for current architecture

**Outgoing:**
- Query execution callbacks (if async queries implemented)
- TBD based on AI agent integration needs

## Semantic Layer

**Components (Planned):**
- Metrics definition layer - Aggregations over base dimensions
- Dimensions - Entities and attributes for filtering/grouping
- Entities - Base objects in the data model
- SQL Translation - Semantic layer query to physical SQL (via DataFusion)
- Tenant Isolation - Query rewriting to inject tenant filters

**Data Sources:**
- Iceberg tables on S3

## AI Agent Integration

**Query Interface:**
- Agents query semantic layer (metrics, dimensions, entities)
- Engine resolves semantic queries to physical SQL
- Results returned as structured data or JSON

**Query Sandboxing:**
- Each agent query runs in ephemeral DataFusion context
- No shared state or resource pools between queries
- Isolated compute prevents resource contention

---

*Integration audit: 2026-02-24*
