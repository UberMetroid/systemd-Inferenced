use tabled::settings::Style;
use tabled::{Table, Tabled};

#[derive(Tabled)]
pub struct PlaneRow {
    #[tabled(rename = "ID")]
    pub id: String,
    #[tabled(rename = "Kind")]
    pub kind: String,
    #[tabled(rename = "Total")]
    pub total: String,
    #[tabled(rename = "Available")]
    pub available: String,
    #[tabled(rename = "Triage Enclave")]
    pub triage: String,
}

#[derive(Tabled)]
pub struct LeaseRow {
    #[tabled(rename = "Lease ID")]
    pub id: String,
    #[tabled(rename = "Plane")]
    pub plane: String,
    #[tabled(rename = "Memory")]
    pub memory: String,
    #[tabled(rename = "Priority")]
    pub priority: String,
    #[tabled(rename = "State")]
    pub state: String,
    #[tabled(rename = "Unit")]
    pub unit: String,
}

/// Format byte counts into human-readable strings (B, KB, MB, GB, TB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let b = bytes as f64;
    if b >= TB {
        format!("{:.2} TB", b / TB)
    } else if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// Apply rounded border style to tabled output tables.
pub fn apply_table_style(table: &mut Table) {
    table.with(Style::rounded());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1024 * 1024 * 50), "50 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 4), "4.0 GB");
    }
}
