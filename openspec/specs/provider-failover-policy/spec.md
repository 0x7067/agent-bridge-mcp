# provider-failover-policy Specification

## Purpose

Define provider attempt classification and bounded router failover rules so the
router may recover from infrastructure failures without silently changing semantic
answers or blockers.

## Requirements

### Requirement: Router classifies each provider attempt before fallback

The system SHALL classify each provider attempt into a router disposition before
deciding whether another provider may run.

#### Scenario: Trusted finality
- **WHEN** an attempt produces provider-authored final text
- **THEN** the router classifies the attempt as trusted finality.
- **AND** it does not run a fallback provider for that prompt turn.

#### Scenario: Failover-eligible failure
- **WHEN** an attempt fails before trusted finality because of infrastructure,
  readiness, or lifecycle failure
- **THEN** the router may classify it as failover-eligible.

#### Scenario: Blocker
- **WHEN** an attempt ends because of auth failure, billing failure, user
  cancellation, explicit provider refusal, or equivalent semantic blocker
- **THEN** the router classifies it as a blocker.
- **AND** it does not silently run a fallback provider.

#### Scenario: Terminal failure
- **WHEN** an attempt fails before finality but policy does not allow fallback or
  no fallback candidate is ready
- **THEN** the router returns a classified terminal failure.

### Requirement: Automatic failover is pre-finality only

The system SHALL allow automatic failover only before trusted finality and only
for failover-eligible infrastructure, readiness, or lifecycle failures.

#### Scenario: Launch failure may fail over
- **WHEN** the first selected provider fails to start before producing final text
  and the fallback provider is ready
- **THEN** the router may run the fallback provider.

#### Scenario: Readiness failure may fail over
- **WHEN** policy selects a provider whose bounded readiness state is not
  launchable before a prompt attempt starts
- **THEN** the router may choose a ready fallback provider.

#### Scenario: Lifecycle failure may fail over
- **WHEN** an attempt closes stdout, exits, or times out before trusted finality
  with a lifecycle-classified failure
- **THEN** the router may run the fallback provider.

#### Scenario: Completed answer does not fail over
- **WHEN** an attempt produces a completed provider-authored answer
- **THEN** the router returns that answer without asking another provider for a
  second answer.

### Requirement: Semantic blockers never silently fail over

The system SHALL NOT silently fail over from refusal, cancellation, auth failure,
billing failure, or equivalent semantic blocker to another provider.

#### Scenario: Explicit refusal
- **WHEN** a provider returns an explicit refusal
- **THEN** the router returns the refusal classification.
- **AND** no fallback provider is launched for that prompt turn.

#### Scenario: User cancellation
- **WHEN** a provider attempt is cancelled by user action or ACP stop reason
- **THEN** the router returns the cancellation classification.
- **AND** no fallback provider is launched for that prompt turn.

#### Scenario: Auth or billing blocker
- **WHEN** provider diagnostics classify the attempt as authentication or billing
  failure
- **THEN** the router returns the blocker.
- **AND** no fallback provider is launched for that prompt turn.

### Requirement: Failover is visible in diagnostics

The system SHALL record every automatic failover in routed-turn diagnostics and
evidence references.

#### Scenario: Failover occurs
- **WHEN** the router runs a fallback provider after a failover-eligible first
  attempt
- **THEN** the final result includes the source provider, target provider,
  source attempt id, target attempt id, failure category, and failover reason.

#### Scenario: No failover occurs
- **WHEN** the router returns trusted finality, a blocker, or a terminal failure
  without fallback
- **THEN** diagnostics do not imply that another provider was consulted.

### Requirement: Provider readiness is policy-visible

The system SHALL use bounded provider readiness as an explicit router policy
input rather than treating static provider availability as launchability.

#### Scenario: Provider is version-only
- **WHEN** a provider has binary/version availability but no startup-verified
  readiness for the selected profile
- **THEN** the router policy does not treat it as launchable by default.

#### Scenario: Provider is smoke-verified
- **WHEN** a provider has bounded smoke-verified readiness for the selected
  profile
- **THEN** router policy may treat it as launchable.
