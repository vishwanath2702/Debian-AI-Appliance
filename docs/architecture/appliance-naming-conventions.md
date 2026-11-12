# Loom Appliance Family — Naming and Codename Convention

**Version:** 1.0  
**Status:** Approved naming convention; future appliances are planned, not yet confirmed as built.

## Purpose

Loom is the shared Rust-based engine used to construct Debian-based appliances. Each appliance product has its own descriptive product name and a stable, memorable codename.

The AI appliance's established codename is **Pragna (ಪ್ರಜ್ಞಾ)**. In this project we use its Kannada sense **“Aware”**, as chosen by the project owner.

## Approved appliance codenames

| Appliance family | Codename | Intended meaning | Status |
|---|---|---|---|
| Debian AI Appliance (DAIA) | **Pragna** | Aware | Existing DAIA identity |
| Storage appliance | **Nidhi** | Treasure | Reserved for future use |
| Network appliance | **Setu** | Bridge | Reserved for future use |
| Security appliance | **Raksha** | Protection | Reserved for future use |
| Development appliance | **Srijana** | Creation | Reserved for future use |
| Media appliance | **Taranga** | Wave | Reserved for future use |
| Backup and recovery appliance | **Smriti** | Memory / remembrance | Reserved for future use |

The English glosses are intended naming associations, not formal linguistic definitions. Kannada spelling and usage for the future names should be reviewed before preparing public-facing branding.

## Naming rules

1. **Loom** names the shared engine, not an individual appliance ISO.
2. Each appliance has a **descriptive product name** and its **own codename**.
3. Codenames are **persistent appliance identities**, not names replaced with each major release.
4. **Release versions are separate from codenames**. For example, `DAIA Pragna 1.0` is an illustrative product/version label, not a claim that version 1.0 has shipped.
5. The future codenames are **reserved names only**. Their inclusion here does not imply that their ISOs, manifests, installers, services, or releases exist.
6. Actual ISO filenames, architecture suffixes, version numbering, and release-channel syntax will be specified when release packaging is designed and verified. Avoid inventing those conventions prematurely.

## Documentation and implementation

Use these names consistently in future appliance profiles, release notes, installer presentation, and ISO metadata **when those appliances are implemented**. Retain separate declarative manifests for each appliance's payload and capabilities rather than hardcoding codename-specific behavior into Loom.

See also the [Loom & DAIA Product Overview](loom-daia-product-overview.md) and the [Technical Glossary](../glossary.md).
