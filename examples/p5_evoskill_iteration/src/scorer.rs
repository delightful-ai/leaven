const TOLERANCES: [f64; 5] = [0.05, 0.01, 0.10, 0.0, 0.025];

pub fn multi_tolerance_score(ground_truth: &str, predicted: &str) -> f64 {
    let mut weighted_sum = 0.0;
    let mut weight_total = 0.0;
    for tolerance in TOLERANCES {
        let weight = 1.0 / (1.0 + 20.0 * tolerance);
        weighted_sum += weight * exact_or_numeric_score(ground_truth, predicted, tolerance);
        weight_total += weight;
    }
    weighted_sum / weight_total
}

pub fn exact_or_numeric_score(ground_truth: &str, predicted: &str, tolerance: f64) -> f64 {
    if predicted.trim().is_empty() {
        return 0.0;
    }
    let gt_numbers = numbers(ground_truth);
    let pred_numbers = numbers(predicted);
    if let (Some(gt), Some(pred)) = (gt_numbers.first(), pred_numbers.first()) {
        if *gt == 0.0 {
            return f64::from(*pred == 0.0);
        }
        let diff = (gt - pred).abs() / gt.abs();
        return f64::from(diff <= tolerance);
    }
    let gt = normalize_text(ground_truth);
    let pred = normalize_text(predicted);
    f64::from(!gt.is_empty() && pred.contains(&gt))
}

fn numbers(text: &str) -> Vec<f64> {
    text.split(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == ','))
        .filter_map(|raw| {
            let normalized = raw.replace(',', "");
            if normalized.is_empty() || normalized == "-" || normalized == "." {
                None
            } else {
                normalized.parse::<f64>().ok()
            }
        })
        .collect()
}

fn normalize_text(text: &str) -> String {
    text.trim()
        .to_ascii_lowercase()
        .replace(['"', '\''], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
