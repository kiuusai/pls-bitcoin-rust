pub fn combine<T: Clone>(items: &[T], size: usize) -> Vec<Vec<T>> {
    if size == 0 {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut current = Vec::with_capacity(size);

    generate_combinations(items, size, 0, &mut current, &mut result);

    result
}

fn generate_combinations<T: Clone>(
    items: &[T],
    size: usize,
    start: usize,
    current: &mut Vec<T>,
    result: &mut Vec<Vec<T>>,
) {
    if current.len() == size {
        result.push(current.clone());
        return;
    }

    for index in start..items.len() {
        current.push(items[index].clone());

        // Only consider later items, preventing reordered duplicates.
        generate_combinations(items, size, index + 1, current, result);

        // Undo the choice before trying the next item.
        current.pop();
    }
}
