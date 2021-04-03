mod backend;
pub mod error;
mod recipe;

use backend::Backend;
use error::EinopsError;
use recipe::{Function, TransformRecipe};

#[derive(Copy, Clone, Debug)]
pub enum Operation {
    Min,
    Max,
    Sum,
    Mean,
    Prod,
}

#[derive(Debug)]
pub struct Rearrange {
    recipe: TransformRecipe,
}

impl Rearrange {
    pub fn new(pattern: &str) -> Result<Self, EinopsError> {
        let recipe = TransformRecipe::new(pattern, Function::Rearrange, None)?;

        Ok(Self { recipe })
    }

    pub fn with_lengths(
        pattern: &str,
        axes_lengths: &[(&str, usize)],
    ) -> Result<Self, EinopsError> {
        let recipe = TransformRecipe::new(pattern, Function::Rearrange, Some(axes_lengths))?;

        Ok(Self { recipe })
    }

    pub fn apply<T: Backend>(&self, tensor: &T) -> Result<T, EinopsError> {
        self.recipe.apply(tensor)
    }
}

#[derive(Debug)]
pub struct Reduce {
    recipe: TransformRecipe,
}

impl Reduce {
    pub fn new(pattern: &str, operation: Operation) -> Result<Self, EinopsError> {
        let recipe = TransformRecipe::new(pattern, Function::Reduce(operation), None)?;

        Ok(Self { recipe })
    }

    pub fn with_lengths(
        pattern: &str,
        operation: Operation,
        axes_lengths: &[(&str, usize)],
    ) -> Result<Self, EinopsError> {
        let recipe =
            TransformRecipe::new(pattern, Function::Reduce(operation), Some(axes_lengths))?;

        Ok(Self { recipe })
    }

    pub fn apply<T: Backend>(&self, tensor: &T) -> Result<T, EinopsError> {
        self.recipe.apply(tensor)
    }
}

#[derive(Debug)]
pub struct Repeat {
    recipe: TransformRecipe,
}

impl Repeat {
    pub fn new(pattern: &str) -> Result<Self, EinopsError> {
        let recipe = TransformRecipe::new(pattern, Function::Repeat, None)?;

        Ok(Self { recipe })
    }

    pub fn with_lengths(
        pattern: &str,
        axes_lengths: &[(&str, usize)],
    ) -> Result<Self, EinopsError> {
        let recipe = TransformRecipe::new(pattern, Function::Repeat, Some(axes_lengths))?;

        Ok(Self { recipe })
    }

    pub fn apply<T: Backend>(&self, tensor: &T) -> Result<T, EinopsError> {
        self.recipe.apply(tensor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tch::{Device, Kind, Tensor};

    #[test]
    fn collapsed_ellipsis_error() {
        let patterns = &["a b c d (...) -> a b c ... d", "(...) -> (...)"];

        for pattern in patterns {
            assert!(Rearrange::new(pattern).is_err());
        }
    }

    #[test]
    fn rearrange_consistency() -> Result<(), EinopsError> {
        let input = Tensor::arange(1 * 2 * 3 * 5 * 7 * 11, (Kind::Float, Device::Cpu))
            .reshape(&[1, 2, 3, 5, 7, 11]);

        let output = Rearrange::new("a b c d e f -> a (b) (c d e) f")?.apply(&input)?;
        assert_eq!(
            input.flatten(0, input.size().len() as i64 - 1),
            output.flatten(0, output.size().len() as i64 - 1)
        );

        let output1 = Rearrange::new("a b c d e f -> f e d c b a")?.apply(&input)?;
        let output2 = Rearrange::new("f e d c b a -> a b c d e f")?.apply(&input)?;
        assert_eq!(output1, output2);

        let rearrange1 = Rearrange::new("a b c d e f -> (f d) c (e b) a")?;
        let rearrange2 =
            Rearrange::with_lengths("(f d) c (e b) a -> a b c d e f", &[("b", 2), ("d", 5)])?;
        let output = rearrange2.apply(&rearrange1.apply(&input)?)?;
        assert_eq!(output, input);

        Ok(())
    }

    #[test]
    fn identity_patterns() -> Result<(), EinopsError> {
        let patterns = &[
            "... -> ...",
            "a b c d e -> a b c d e",
            "a b c d e ... -> ... a b c d e",
            "a b c d e ... -> a ... b c d e",
            "... a b c d e -> ... a b c d e",
            "a ... e -> a ... e",
            "a ... -> a ...",
            "a ... c d e -> a (...) c d e",
        ];

        let input = Tensor::arange(2 * 3 * 4 * 5 * 6, (Kind::Float, Device::Cpu))
            .reshape(&[2, 3, 4, 5, 6]);

        for pattern in patterns {
            assert_eq!(
                input,
                Rearrange::new(pattern)?.apply(&input)?,
                "{} failed",
                pattern
            );
        }

        Ok(())
    }

    #[test]
    fn equivalent_rearrange_patterns() -> Result<(), EinopsError> {
        let patterns = &[
            ("a b c d e -> (a b) c d e", "a b ... -> (a b) ..."),
            ("a b c d e -> a b (c d) e", "... c d e -> ... (c d) e"),
            ("a b c d e -> a b c d e", "... -> ..."),
            ("a b c d e -> (a b c d e)", "... -> (...)"),
            ("a b c d e -> b (c d e) a", "a b ... -> b (...) a"),
            ("a b c d e -> b (a c d) e", "a b ... e -> b (a ...) e"),
        ];

        let input = Tensor::arange(2 * 3 * 4 * 5 * 6, (Kind::Float, Device::Cpu))
            .reshape(&[2, 3, 4, 5, 6]);

        for (pattern1, pattern2) in patterns {
            let output1 = Rearrange::new(pattern1)?.apply(&input)?;
            let output2 = Rearrange::new(pattern2)?.apply(&input)?;

            assert_eq!(output1, output2);
        }

        Ok(())
    }

    #[test]
    fn equivalent_reduction_patterns() -> Result<(), EinopsError> {
        let patterns = &[
            ("a b c d e -> ", "... -> "),
            ("a b c d e -> (e a)", "a ... e -> (e a)"),
            ("a b c d e -> d (a e)", "a b c d e ... -> d (a e)"),
            ("a b c d e -> (a b)", "... c d e -> (...)"),
        ];
        let operations = &[Operation::Sum, Operation::Min, Operation::Max];

        let input = Tensor::arange(2 * 3 * 4 * 5 * 6, (Kind::Float, Device::Cpu))
            .reshape(&[2, 3, 4, 5, 6]);

        for operation in operations {
            for (pattern1, pattern2) in patterns {
                let output1 = Reduce::new(pattern1, *operation)?.apply(&input)?;
                let output2 = Reduce::new(pattern2, *operation)?.apply(&input)?;

                assert_eq!(output1, output2);
            }
        }

        Ok(())
    }
}
