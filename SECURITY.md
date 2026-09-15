# Security Policy

## Supported versions

Ferrite is pre-1.0. Security fixes are applied to the latest released minor
line; older pre-release versions are not maintained.

| Version | Supported |
| --- | --- |
| 0.1.x | Yes |
| < 0.1 | No |

## Reporting a vulnerability

Please **do not open a public issue** for security problems.

Report vulnerabilities privately through GitHub's
[private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability):

1. Go to the repository's **Security** tab.
2. Click **Report a vulnerability**.
3. Describe the issue using the details below.

If you cannot use GitHub, contact the maintainers through the repository's
profile and request a private channel before sharing any details.

### What to include

- A description of the vulnerability and its impact.
- Affected crate(s), version(s), and feature flags.
- A minimal reproduction or proof of concept.
- Any known mitigations or workarounds.
- Your assessment of severity, if you have one.

## What to expect

- **Acknowledgement** within 3 business days.
- **Initial assessment** and severity triage within 7 business days.
- **Coordinated disclosure**: we will agree on a disclosure timeline with you
  and credit you in the advisory unless you prefer to stay anonymous.

These timelines are best-effort targets for a volunteer-maintained project.

## Scope

In scope:

- All `ferrite-*` crates in this repository.
- The `fr` CLI and generated project templates.
- CI/CD workflows and release tooling in `.github/`.

Out of scope:

- Vulnerabilities in third-party dependencies — report those upstream
  (Dependabot and `cargo audit` already track them here).
- Issues that require a compromised local machine, leaked secrets, or
  non-default, clearly documented insecure configuration.
- Denial-of-service from running untrusted code that the framework never
  intended to sandbox.

## Automated scanning

This repository runs the following checks on every push and pull request:

- **`cargo audit`** for known vulnerabilities in the dependency tree.
- **gitleaks** to detect committed secrets.
- **CodeQL** static analysis for the Rust codebase.

Dependencies are kept current with Dependabot.

## Safe harbor

We will not pursue legal action against researchers who make a good-faith effort
to comply with this policy, avoid privacy violations and data destruction, and
give us reasonable time to respond before public disclosure.
