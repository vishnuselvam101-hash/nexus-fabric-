# Licensing Packages — Proposal for Management Approval

18 November 2026 · **Proposed structure only. No prices, discounts, customer numbers or financial claims. Until management approves, every film and page uses "Request a quotation".**

| Package (proposed) | Intended for | Proposed contents | Depends on software that does not exist yet |
|---|---|---|---|
| Engineering Professional | An individual fabrication or CAD engineer; a small drawing office | One workstation; supported geometry families; verification; DXF output | Installer; licensing mechanism |
| Factory | One fabrication plant with several engineers | Shared on-premises server; projects, roles, audit; assemblies and obligations | Multi-user hardening (TLS, database encryption, backups) |
| Enterprise | A company with several plants or engineering teams | Central administration; identity integration; reporting | Single sign-on, multi-factor authentication, tenant separation |
| On-Premises Enterprise | Organisations that cannot use external networks for drawings | Offline installation and updates; customer-held keys | Offline update mechanism; key management |
| Custom Industrial Deployment | Special processes, machines or rules | Factory-specific rule tables, calibration, CAM output profiles, integration work | Calibration tooling; CAM integrations |

## Decisions needed from management

1. Whether these five packages are the right structure.
2. Pricing model (per seat, per server, subscription or perpetual with maintenance) and prices.
3. What is in scope for a first commercial offer, given the feature inventory: today the software cannot read drawings and cannot release a result for manufacturing.
4. Terms for pilots (the master plan proposes shadow-engineering pilots).

## For the pricing film (30–45 seconds, not yet scripted)

Show the five packages as proposed, who each is for, and "Request a quotation". No numbers. A line on screen: "Proposed packages. Availability depends on product development."
