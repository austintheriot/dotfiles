---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# DevOps / Infrastructure-as-Code

A reference for evaluating IaC and operational infrastructure code from a correctness, blast-radius, cost, and disaster-recovery lens. Used by the `devops-infrastructure` subagent.

The scope: **Terraform / OpenTofu / Pulumi / CloudFormation / CDK / Helm / Kustomize / Crossplane / Bicep / ArgoCD / Flux** configs, plus the surrounding operational concerns: state management, drift detection, module design, blast radius, IAM / least privilege, multi-account / multi-region patterns, networking (VPC / subnet / SG / NACL / Transit Gateway / PrivateLink), DNS / certificates, disaster recovery (RTO / RPO, backups, multi-region failover), GitOps patterns, Kubernetes manifest quality (resource limits, probes, anti-affinity, PDBs), platform engineering / IDPs (Backstage), observability of infra.

Distinct from:
- **`ci-pipeline`**: pipeline operational design (workflows, signing, branch protection, OIDC). This agent is the **infrastructure being managed**, not the pipeline that manages it.
- **`security`**: general threat model. We overlap heavily on IAM, secrets in state, network attack surface; we own the IaC-specific angle.
- **`distsys-runtime`**: runtime distributed-systems behavior (retries, queues, cascading). We own the infra config that *shapes* runtime behavior.
- **`distsys-data`**: storage / replication design. We touch IaC provisioning of databases; the design lens is theirs.
- **`performance`**: general perf. We touch cost-shape choices.
- **`observability-practice`**: SLOs, alerting practice. We touch the infra observability surface (CloudTrail / flow logs / cost anomaly detection).

The core thesis: **most production incidents in cloud infrastructure are not novel failures of the cloud provider. They are misconfigurations in IaC, missing controls at provisioning time, blast radius that wasn't bounded, secrets in places they shouldn't be, or operational invariants encoded only in someone's head.** The reviewer's job is to catch these at PR time, when they cost an hour, rather than at 3am, when they cost a week.

The operational thesis: **the Terraform / Pulumi / Helm PR is not "config." It's a program that mutates production infrastructure on merge.** The same code-review discipline applied to application code applies, plus more: the blast radius is wider, the rollback story is harder, and the audit trail is the only record of what happened.

---

## Universal principles

### Every IaC change is a deployment

Every deployment has a blast radius, a reversibility story, and a verification step. If any of the three is unstated, the change is incomplete.

**Flag**: PRs that change infrastructure without naming the blast radius; PRs with no rollback path; PRs without a verification step (`terraform plan` output review, dry-run, staging-first).

### State is the most expensive thing

State corruption is the most expensive failure mode in the IaC stack. State surgery (`terraform state mv`, `terraform state rm`, `terraform import`) is hazardous and often irreversible.

**Flag**: state stored locally for team-shared infra (single-developer pattern); state stored without locking (concurrent applies corrupt state); state in repos (state contains decoded secrets); state surgery without backup / verification; state not versioned (S3 versioning enabled? GCS object versioning enabled?).

### Blast radius is the configuration question

The IaC pyramid:
1. **State**: what the tool believes exists.
2. **Modules**: reusable abstractions.
3. **Root modules**: the unit of `apply`.
4. **Environments**: dev / staging / prod separation.
5. **Organization**: account / project boundaries.

Mismatches in altitude are the recurring failure: one root module managing everything (blast radius = the company); one state file for prod and dev (a typo in dev destroys prod); modules tightly coupled to root concerns.

**Flag**: root modules with hundreds of resources (the apply is unbounded); one state file across environments; cross-environment references that bypass the boundary.

### Pin everything to immutable references

Same principle as `ci-pipeline`: pin to immutable references. Provider versions (`~> 5.0` is acceptable; unconstrained is not). Module sources (`?ref=v1.2.3` not `?ref=main`). Container images (digest, not tag).

**Flag**: modules sourced from Git without `ref=` pinning; providers without `required_providers`; container images by `:latest` or moving tags; Helm chart references without version pinning.

### Least privilege at provisioning time

Every IAM role, every security group, every Kubernetes RBAC binding is a permission grant. The provisioning step is where these are set; reviewing them at provisioning time is cheaper than discovering over-privileged access during an incident.

**Flag**: IAM policies with `Action: "*"` or `Resource: "*"`; security groups with `0.0.0.0/0` ingress on non-public ports; Kubernetes pods running as root without justification; ServiceAccounts with cluster-admin.

---

## Terraform / OpenTofu specifics

### The HashiCorp BUSL fork (August 2023) and OpenTofu

HashiCorp relicensed Terraform under the Business Source License (BUSL); the Linux Foundation forked it as **OpenTofu** (under MPL). Major shops chose sides. Both tools are mostly compatible today but diverge over time.

**Flag**: project using Terraform >= 1.6 commercially without acknowledging the BUSL constraint; project using OpenTofu but pulling provider / module versions that target HashiCorp-only features (or vice versa).

### State management

- **Local state** for any team work: anti-pattern.
- **Remote state**: S3 + DynamoDB lock (the AWS-canonical), GCS (uses object lifecycle for locking), Azure Storage, Terraform Cloud / HCP Terraform, Spacelift, env0, Atlantis.
- **State locking**: required for any team. Concurrent `apply` without lock corrupts state.
- **State versioning**: enable bucket versioning on the state-storing backend.
- **State backup**: state contents include decoded secrets; back up but treat as sensitive.

**Flag**: state stored locally for team infra; state stored without locking; state in a repo (state file with `*.tfstate` checked in); state without bucket versioning.

### Module design

- **Composability**: modules with clear inputs / outputs.
- **Versioning**: tag releases (`v1.2.3`); consumers pin to versions.
- **Composition over inheritance**: nested modules with explicit interfaces.
- **Root vs reusable modules**: a root module wires environments; reusable modules implement abstractions. Don't conflate.
- **The Anton Babenko `terraform-aws-modules` pattern**: opinionated, well-versioned modules; reference quality.

**Flag**: modules without version pinning at the call site (`?ref=main` is a moving target); modules with provider configuration (provider config belongs in root); modules with deep coupling to specific environments; one mega-module that does everything (decompose).

### `for_each` vs `count`

`for_each` over a map is the safe pattern: deleting one element doesn't reindex others. `count` over a list reindexes on delete; instances 3-N get destroyed and recreated. Use `for_each` for resource sets; `count` only for binary enabled/disabled flags.

**Flag**: `for_each` over a list (use a map); `count = var.enabled ? 1 : 0` for resources that change state on toggle (state migration pain when toggled); `count` over an indexable list of resources.

### Lifecycle directives

- **`prevent_destroy = true`**: prevents accidental destruction. Critical resources (databases, KMS keys, S3 buckets with data).
- **`ignore_changes = [...]`**: ignores out-of-band changes. Use sparingly; suppresses drift detection.
- **`create_before_destroy = true`**: zero-downtime replacement. Required for some resource types where in-place update isn't supported.

**Flag**: critical resources (RDS / KMS / S3 with data / IAM roles trusted by other accounts) without `prevent_destroy`; `ignore_changes` on resources where drift matters; missing `create_before_destroy` on resources that recreate on changes (e.g., security groups).

### Sensitive variables and state

`sensitive = true` marks a variable for log redaction but **does not** encrypt it in state. State files contain decoded secrets.

**Flag**: secrets in `*.tf` files (committed to Git); secrets in state without encryption-at-rest on the backend (S3 server-side encryption, KMS); state not access-controlled separately from code; `sensitive = true` treated as "this is secret in state" (it's not).

### Provisioners (`local-exec` / `remote-exec`)

Brittle, hard to debug, out-of-state effects. Last resort.

**Flag**: `local-exec` for anything beyond trivial post-creation hooks; `remote-exec` where the cloud-init / user-data path would work; provisioners performing data migrations or operational tasks that should be a separate runbook.

### Drift detection

`terraform plan` shows drift; refresh-only plans verify state vs reality. Drift accumulates from click-ops, automation outside IaC, or external systems (cloud auto-scaling, ASG modifications).

**Flag**: no drift detection in CI; periodic plans not run; click-ops alongside IaC creating untracked changes.

---

## Pulumi specifics

- **Programming language as IaC**: TypeScript, Python, Go, C#, Java, YAML.
- **Pulumi state**: similar story to Terraform; Pulumi Cloud (managed), self-hosted, S3 backend.
- **Component resources**: reusable abstractions in code.

The Pulumi vs Terraform debate: programming-language flexibility vs HCL declarativeness. Each has cost. The reviewer's stance: match the project's choice; flag inconsistency or anti-pattern within that choice.

**Flag** (Pulumi-specific): components without unit tests (the programming-language benefit unused); side effects outside resource declarations (the declarative invariant violated); secrets used inline without `pulumi.secret()`.

---

## CDK / CloudFormation / Bicep

- **AWS CDK**: TypeScript / Python / Java / C# / Go; generates CloudFormation. Powerful abstraction; CFN is the substrate.
- **CloudFormation**: AWS-native JSON / YAML; the lowest-common-denominator.
- **AWS SAM**: CFN extension for serverless.
- **Bicep**: Azure-preferred IaC; ARM template successor.
- **CDKTF**: CDK syntax generating Terraform.

**Flag**: CDK code with custom resources (Lambda-backed CFN resources) for things vanilla CFN supports; CFN templates manually edited after CDK synthesis (drift); CDK app structure with one massive stack (blast radius); Bicep modules with hard-coded references.

---

## Kubernetes manifests

### Helm and Kustomize

- **Helm 3**: the package manager. Charts, `values.yaml`, templating.
- **Kustomize**: overlay-based; built into `kubectl`. Simpler than Helm for many cases.
- **Crossplane**: K8s API for cloud resources. AWS / GCP / Azure resources as K8s objects.

**Flag**: Helm charts with hardcoded values (templating not used); Kustomize overlays mutating beyond what should be overlay-able (e.g., changing application logic); Helm + Kustomize layered in confusing ways.

### Manifest quality

The recurring K8s anti-patterns:

- **Missing resource requests / limits**: containers are bin-packing hazards; pods get scheduled anywhere; noisy-neighbor effects.
- **Missing liveness / readiness / startup probes**: K8s can't determine pod health; rolling updates send traffic to unready pods.
- **No PodDisruptionBudget (PDB)** on replicated workloads: voluntary disruptions (node drains, upgrades) can take down the entire workload.
- **No anti-affinity**: replicas can co-locate on one node; node failure takes them all out.
- **HPA without metrics**: scales on CPU alone, ignoring real load (queue depth, RPS).
- **No NetworkPolicies** in multi-tenant clusters: default allow-all is the K8s default.
- **Missing PodSecurityStandards (PSS) labels** on namespaces: privilege escalation enabled by default.
- **Hardcoded `:latest`** or moving image tags: deploy state is not pinned.
- **Secrets in env vars from ConfigMaps**: ConfigMaps are not Secrets; both are base64 in etcd but ConfigMaps lack the convention.
- **Missing `securityContext`**: privilege escalation, root user, host-mount defaults.

### Operator pattern and Crossplane

- **Operators** encode operational knowledge as custom controllers; OperatorHub catalogs them.
- **Crossplane** brings cloud resources into the K8s API model.

**Flag**: operators without health metrics; operators reconciling on tight loops (control-plane load); Crossplane composition without provider versioning.

---

## GitOps

### ArgoCD / Flux

- **ArgoCD**: pull-based; declarative; the dominant choice.
- **Flux**: pull-based; CNCF graduated; Weaveworks origin.

**The GitOps principles** (Alexis Richardson 2017): declarative, versioned, automated, observable.

**Flag**: manual `kubectl apply` alongside ArgoCD / Flux (silent cluster drift); secrets in Git unencrypted (use Sealed Secrets, SOPS, or External Secrets); ArgoCD with cluster-admin (use namespace-scoped); no automated drift remediation; untemplated environment-specific values in Git; GitOps controller without RBAC bounds.

### Push vs pull deploys

GitOps is pull (cluster watches Git); CD systems are typically push. Mixing them without clear ownership leads to "who deployed this" confusion.

**Flag**: push CD systems (Spinnaker, Jenkins) deploying to clusters managed by GitOps controllers without explicit coordination.

---

## Cloud platforms

### AWS / GCP / Azure / Cloudflare / others

- **AWS**: broadest surface; the default. Strong on storage, compute primitives, managed services.
- **GCP**: strong on data / ML / networking; smaller market share.
- **Azure**: Microsoft-shop integration.
- **Cloudflare**: edge compute (Workers), DNS, CDN, R2 (S3-compatible), Durable Objects.
- **Fastly**: Compute@Edge (WASM), CDN.
- **Vercel / Netlify**: managed frontend hosting.
- **DigitalOcean / Linode / Hetzner / OVH**: cost-effective hyperscalers for simpler workloads.
- **Render / Fly / Railway**: managed app platforms.

**Flag**: multi-cloud for the sake of it (the cost is real; the leverage is often illusory); choosing the wrong tier (Hetzner for a Fortune 500 compliance shop; AWS Enterprise Support for a side project).

---

## Networking

### VPC design

- **CIDR planning**: don't pick `10.0.0.0/16` if you might peer with another `10.0.0.0/16`. Use RFC 1918 with /16 or larger ranges.
- **Subnet sizing**: public vs private; one per AZ minimum for HA.
- **NAT gateways**: cost-attribution surface (NAT egress charges per GB).
- **Transit Gateway** vs **VPC Peering**: TGW for hub-and-spoke; peering for one-off.
- **PrivateLink / VPC Endpoints**: private connectivity to managed services without internet routing. Critical for security and often for cost (S3 Gateway endpoints save NAT egress).

### Security groups vs NACLs

- **Security groups**: stateful, the workhorse, ingress and egress at the instance level.
- **NACLs**: stateless, subnet-level, additional defense layer.

**Flag**: security groups with `0.0.0.0/0:22` (SSH from anywhere); SG rules referencing IP ranges instead of other SGs (use SG-to-SG references); missing egress rules (default allow-all); single-AZ deployment for production; all-traffic-through-NAT when S3 Gateway endpoints would save egress; VPC peering at scale (use TGW).

---

## IAM / least privilege

### AWS IAM

- **Roles**: assumed by services or users; the modern pattern.
- **Permanent access keys**: legacy; avoid for human users (use IAM Identity Center / SSO).
- **Permission boundaries**: hard cap on what a role can grant or do.
- **Resource-based policies**: bucket policies, KMS key policies; allow cross-account access.
- **IMDSv2**: required (IMDSv1 enables SSRF).
- **Service Control Policies (SCPs)** at the AWS Organizations level: organization-wide guardrails.

### Identity providers

- **AWS IAM Identity Center** (formerly SSO): the AWS-recommended for human access.
- **OIDC trust** for CI / external systems: short-lived credentials, no static keys.
- **Workload Identity** (GCP) / **Managed Identity** (Azure) / **IRSA** (EKS) / **Workload Identity for GKE**: pod-level identity without keys.

**Flag**: IAM policies with wildcards (`Action: "*"`, `Resource: "*"`); permanent access keys for any non-emergency-glass-break human; missing IMDSv2 enforcement; resource-based policies without `aws:PrincipalOrgID` condition (cross-org confusion); SCPs missing on Organizations setups; long-lived service-account keys when Workload Identity / IRSA would work.

---

## Secrets management

- **Vault** (HashiCorp): secrets store; dynamic credentials; PKI; the most general.
- **AWS Secrets Manager / Parameter Store**: AWS-native.
- **GCP Secret Manager** / **Azure Key Vault**: cloud-native.
- **External Secrets Operator** (K8s): syncs from external stores into K8s Secrets.
- **Sealed Secrets** (Bitnami): encrypted K8s Secrets storable in Git.
- **SOPS** (Mozilla): encrypted YAML / JSON / INI / ENV files.

**Flag**: secrets in Git unencrypted; secrets in IaC variables stored without encryption-at-rest backend; secrets in K8s ConfigMaps (use Secrets); long-lived secrets without rotation; SealedSecrets without key-rotation cadence.

---

## Cost / FinOps

### Tagging strategy

Cost attribution requires consistent tagging. The standard tag set: `Environment`, `Team`, `Project`, `Owner`, `CostCenter`. Required at provisioning time; retrofitting tags is painful.

**Flag**: missing tagging strategy; tags without enforcement (SCPs or aws_organizations_policy denying creation without required tags); untagged resources accumulating (orphan spend).

### Compute optimization

- **Spot / preemptible**: ~70% discount for interruption-tolerant workloads.
- **Reserved instances / Savings Plans**: 1-3 year commitments for ~30-60% off the predictable baseline.
- **Autoscaling**: HPA / VPA / Cluster Autoscaler / **Karpenter** (AWS, the modern replacement).
- **Right-sizing**: CPU / memory utilization < 20% is the over-provisioning signal.

**Flag**: idle resources (24/7 dev environments without auto-stop); oversized instances; missing reserved capacity for predictable baseline; spot for stateful workloads without checkpointing; HPA without sensible scale-down thresholds (thrashing); no cost anomaly detection.

---

## Disaster recovery

### RTO / RPO

- **RTO** (Recovery Time Objective): how long can we be down?
- **RPO** (Recovery Point Objective): how much data can we lose?

Both must be documented per-system; the IaC and operational design must support them.

### Backup strategy

- **Snapshot frequency**: matches RPO.
- **Retention**: balances cost and historical needs.
- **Cross-region replication**: for region-failure DR.
- **Encryption**: at rest, with per-account / per-organization KMS keys.
- **Tested restoration**: backups untested are theater.

**Flag**: no documented RTO / RPO; backups untested; single-region for production; manual failover (not automated where possible); no backup of state files themselves; no DR drill cadence.

---

## Observability of infrastructure

- **CloudTrail** (AWS) / **Cloud Audit Logs** (GCP) / **Azure Activity Log**: API call history. Required for any compliance.
- **VPC Flow Logs**: network traffic record.
- **Cost anomaly detection**: AWS Cost Anomaly Detection, GCP Recommender.
- **Resource events**: CloudWatch Events / EventBridge, Cloud Audit, Azure Event Grid.

**Flag**: CloudTrail / Audit Logs disabled; flow logs disabled; no alerting on infrastructure events (root account login, IAM policy changes, S3 bucket policy changes); no cost anomaly detection.

---

## Platform engineering

- **Internal developer platforms (IDPs)**: Backstage (Spotify, CNCF), Humanitec, port.io, Cycle.io.
- **Team Topologies** (Skelton & Pais): stream-aligned + platform team model. Platform team builds the IDP; stream-aligned teams consume.
- **Golden paths**: opinionated defaults; self-service provisioning within guardrails.

**Flag**: platform team without consumer feedback loop; IDP as a project rather than a product (no roadmap, no SLAs); golden paths that consumers route around because they're inadequate.

---

## Anti-pattern catalog

### Terraform / IaC
- State stored locally for team infra.
- State stored without locking.
- Secrets in `*.tf` files.
- `sensitive = true` treated as encryption-at-rest (it isn't).
- `local-exec` for anything non-trivial.
- Modules without `?ref=` version pinning.
- Root modules with hundreds of resources.
- One state file for everything.
- `terraform apply -auto-approve` in interactive paths.
- Missing `prevent_destroy` on critical resources.
- `count = var.enabled ? 1 : 0` for stateful resources.
- `for_each` over a list (use a map).
- Provider configuration in modules.
- No drift detection in CI.

### IAM / security
- Wildcards in IAM policies.
- Permanent access keys for humans.
- IMDSv1 enabled.
- S3 buckets without `BlockPublicAccess`.
- Security groups with `0.0.0.0/0` ingress on non-public ports.
- K8s pods running as root.
- Privileged containers without justification.
- Missing NetworkPolicies in multi-tenant clusters.
- Missing SCPs at the Organizations level.

### Kubernetes manifests
- Missing resource requests / limits.
- Missing liveness / readiness / startup probes.
- No PDB on replicated workloads.
- No anti-affinity on multi-replica services.
- Hardcoded image tags or `:latest`.
- Secrets in env vars from ConfigMaps.
- Missing `securityContext`.
- HPA without sensible metrics.
- Missing PodSecurityStandards labels.

### Networking
- Single-AZ production.
- All traffic through NAT when PrivateLink / S3 Gateway would work.
- Missing VPC flow logs.
- VPC peering at scale (use TGW).
- Manual certificate renewal (use ACM / cert-manager).

### Cost
- No cost-allocation tagging.
- Untagged resources accumulating.
- Idle resources (24/7 dev environments).
- Oversized instances.
- No reserved capacity for predictable baseline.
- Spot for stateful workloads without checkpointing.
- No cost anomaly detection.

### Disaster recovery
- No documented RTO / RPO.
- Backups untested.
- Single-region for production.
- Manual failover.
- No backup of state files.
- No DR drill cadence.

### GitOps
- Manual `kubectl apply` alongside ArgoCD / Flux.
- Secrets in Git unencrypted.
- Untemplated env-specific values.
- ArgoCD / Flux with cluster-admin.
- No automated drift remediation.

### Observability of infra
- CloudTrail / Audit Logs disabled.
- VPC flow logs disabled.
- No alerting on infra events.
- No cost anomaly detection.

### Process
- Untested IaC merged (no `terraform plan` in CI).
- IaC changes not reviewed (no CODEOWNERS).
- Manual infrastructure changes outside IaC (click-ops drift).
- No infrastructure change log.

---

## Modern shifts (2024-2026)

- **OpenTofu** as the open-source Terraform fork after BUSL (Aug 2023). Adoption growing; some major shops migrated, others stayed.
- **Pulumi 3.x** mature; the programming-language-as-IaC option.
- **Crossplane** as Kubernetes-native cloud resource management.
- **Backstage** as the dominant IDP framework.
- **Platform Engineering** as a recognized discipline (vs DevOps as cultural movement).
- **Karpenter** (AWS) replacing Cluster Autoscaler for K8s autoscaling.
- **Gateway API** replacing Ingress in K8s.
- **Cilium / eBPF** networking gaining mainstream adoption.
- **Wasm-on-edge** (Fastly Compute, Fermyon Spin, Cloudflare Workers WASM).
- **FinOps maturity**: the FinOps Foundation, cost-aware engineering.
- **SLSA / Sigstore / supply chain** for IaC artifacts (signed Terraform providers, signed Helm charts).
- **AI-assisted IaC**: Copilot for Terraform, Claude for cloud architecture. Real adoption; questionable correctness without review.
- **Multi-cloud reality check**: most shops are mono-cloud or 80/20. True multi-cloud is rare and costly.

---

## Schools of disagreement (preserve)

- **Terraform vs OpenTofu**: post-BUSL, real. Major shops have chosen sides.
- **Terraform vs Pulumi**: HCL declarativeness vs programming-language flexibility.
- **CDK vs CloudFormation directly**: CDK abstracts; CFN is the substrate.
- **Helm vs Kustomize**: templating vs overlay. Both ship.
- **GitOps (ArgoCD / Flux) vs traditional CD**: pull vs push.
- **Multi-cloud vs cloud-native**: portability vs leverage.
- **Operator pattern vs imperative scripts**: declarative reconciliation vs imperative deploys.
- **Service mesh vs library-based service connectivity**: Istio / Linkerd vs in-app SDK.
- **Backstage vs build-your-own IDP**.

---

## What is NOT a devops-infrastructure finding

- **Pipeline operational design** (CI workflows, signing, branch protection, OIDC): route to `ci-pipeline`.
- **General security threat model**: route to `security`. We own IaC-specific (IAM scope, secrets in state, network attack surface from VPC config).
- **Runtime distributed-systems behavior** (retries, queues, cascading): route to `distsys-runtime`.
- **Storage / replication design** (DB engine choice, sharding, isolation): route to `distsys-data`. We touch IaC provisioning.
- **Application-level performance**: route to `performance`. We touch cost-shape choices.
- **SLO / alerting practice / on-call**: route to `observability-practice`. We touch infra-event observability.
- **Application code**: route to language-specific agents.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: state stored without locking on team-shared infra (concurrent applies corrupt state); IAM policy with `Action: "*"` `Resource: "*"` on a privileged role; secrets committed to Git unencrypted; security group with `0.0.0.0/0:22` or `:3306` ingress; `prevent_destroy` missing on a production database; `terraform apply -auto-approve` without staging gate on production; CloudTrail disabled on production accounts.
- **major**: modules without version pinning at call site; root module managing too much (blast radius); one state file across environments; Kubernetes pods without resource limits / probes / PDB / anti-affinity in production; missing PSS labels on namespaces; IMDSv1 enabled; single-AZ production; no DR drill cadence or RTO / RPO documented; secrets in K8s ConfigMaps; manual `kubectl apply` alongside ArgoCD; untested IaC merged without `terraform plan` in CI; no cost anomaly detection on accounts > $10k/month.
- **minor**: missing tagging strategy; oversized instances; idle dev environments; HPA without sensible thresholds; suboptimal NAT routing when S3 Gateway endpoints would save egress.
- **nit**: variable naming inconsistencies; module structure preferences; minor cost optimizations.
- **insight**: structural -- "the platform team would benefit from Backstage at this scale"; "consider Karpenter for K8s autoscaling at this node count"; "this codebase has accreted three IaC tools; consider consolidating"; "Crossplane would simplify the K8s + cloud-resource composition story."

Confidence: high when the trigger is concrete (a specific policy with `*`, a specific resource without `prevent_destroy`); medium when reasoned from architecture (a pattern that the agent infers from one file).

---

## Process for the devops-infrastructure agent

1. **Identify the IaC tool(s).** Terraform / OpenTofu / Pulumi / CloudFormation / CDK / Bicep / Helm / Kustomize / Crossplane / ArgoCD / Flux?
2. **Identify the cloud(s).** AWS / GCP / Azure / Cloudflare / multi? The IAM and networking patterns differ.
3. **Read project conventions.** `docs/infrastructure.md`, `CLAUDE.md` infra sections, the module / environment structure.
4. **Walk state management.** Locking? Versioning? Encryption-at-rest? Not in Git? Not local?
5. **Walk module quality.** Versioning at call site? Inputs / outputs clean? Providers in root, not modules?
6. **Walk IAM.** Wildcards? Permanent keys? IMDSv2? Least privilege visible?
7. **Walk Kubernetes manifests** (if applicable). Resource limits? Probes? PDB? Anti-affinity? `securityContext`? Pinned images?
8. **Walk networking.** Multi-AZ? Egress paths (NAT vs PrivateLink)? Security group rules? Flow logs?
9. **Walk secrets.** In Git? In state? In ConfigMaps? Rotation?
10. **Walk cost.** Tagging strategy? Reserved capacity? Idle resources? Anomaly detection?
11. **Walk DR.** RTO / RPO documented? Backup tested? Multi-region story?
12. **Walk GitOps** (if applicable). Manual apply alongside controller? Secrets unencrypted in Git? Drift remediation?
13. **Walk observability of infra.** CloudTrail / flow logs / cost alarms enabled?
14. **Route to other lenses** where the angle is theirs: pipeline → `ci-pipeline`; general security → `security`; runtime distsys → `distsys-runtime`; storage design → `distsys-data`; app perf → `performance`; SLO practice → `observability-practice`.
15. **Stay read-only.**
