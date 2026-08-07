use crate::NoteColumn;

/// Deterministic horizontal justification with minimum collision spacing.
pub fn justify_columns(columns: &[NoteColumn], left: f32, width: f32) -> Vec<f32> {
    if columns.is_empty() {
        return Vec::new();
    }
    let minimums: Vec<f32> = columns
        .iter()
        .map(|column| {
            if column.rest {
                14.0
            } else {
                16.0 + column.notes.len().saturating_sub(1) as f32 * 3.0
            }
        })
        .collect();
    let minimum_total: f32 = minimums.iter().sum();
    let total_ticks: u64 = columns.iter().map(|column| column.duration_ticks).sum::<u64>().max(1);
    let extra = (width - minimum_total).max(0.0);
    let mut x = left + minimums[0] * 0.5;
    let mut result = Vec::with_capacity(columns.len());
    for (index, column) in columns.iter().enumerate() {
        result.push(x);
        if let Some(next) = minimums.get(index + 1) {
            x += minimums[index] * 0.5 + next * 0.5 + extra * (column.duration_ticks as f32 / total_ticks as f32);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DurationValue, NoteColumn};
    #[test]
    fn preserves_minimum_column_gaps() {
        let columns = (0..3)
            .map(|index| NoteColumn {
                start_tick: index * 120,
                duration_ticks: 120,
                duration: DurationValue::Sixteenth,
                notes: vec![],
                rest: false,
            })
            .collect::<Vec<_>>();
        let x = justify_columns(&columns, 0.0, 120.0);
        assert!(x[1] - x[0] >= 16.0);
        assert!(x[2] - x[1] >= 16.0);
    }
}
