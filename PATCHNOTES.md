# v2.2.1 Patch Notes

## Enhanced Transparency & Detailed Reporting

This release focuses on providing a professional, "serious mode" level of technical transparency during package operations.

### Key Changes:

*   **Detailed Installation Summary**: The install/upgrade prompt now includes a comprehensive table showing:
    *   Target Repository.
    *   Package Licenses.
    *   Detailed Weights (Download vs. Installed size).
*   **Transaction Live Monitoring**: Integrated ALPM event callbacks to provide real-time updates during the "committing transaction" phase:
    *   Live progress for individual package operations (Installing, Upgrading, Removing).
    *   Accurate indexing (e.g., `(1/5) upgrading linux...`).
    *   Transparent Hook Monitoring showing specifically which backend hooks are running.
*   **Clean Output Overhaul**: Removed more informal language ("please", "successfully") and debug-style prints for a more concise and professional CLI experience.
*   **API Stability**: Fixed internal ALPM API handling to ensure reliable transaction commit reporting across different system configurations.
*   **Optimization**: Slightly refined the Turbo Engine's parallel segment racing for even more consistent throughput on high-bandwidth connections.

---
*pacboost - High-performance package management for Arch Linux.*
