pub mod config;
pub mod deck_of_cards;
pub mod model;
pub mod mongo;
pub mod state;

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use mockall::{automock, predicate};
    use std::num::NonZeroUsize;
    use test_case::test_case;

    #[automock]
    #[async_trait]
    trait DoesSomething {
        fn method(&self, a: usize, b: usize) -> usize;
        async fn method_async(&self, c: &str, d: &str) -> usize;
    }

    fn my_func<D: DoesSomething>(d: &D) -> usize {
        d.method(2, 3)
    }

    #[test]
    fn mocked() {
        let mut d = MockDoesSomething::new();
        d.expect_method()
            .with(predicate::eq(2), predicate::eq(3))
            .returning(|a, b| a + b)
            .once();

        assert_eq!(5, my_func(&d));
    }

    async fn my_func_async<D: DoesSomething>(d: &D) -> usize {
        d.method_async("hello", "world!").await
    }

    #[tokio::test]
    async fn mocked_async() {
        let mut d = MockDoesSomething::new();
        d.expect_method_async()
            .with(
                predicate::function(|s: &str| s.len() == 5),
                predicate::eq("world!"),
            )
            .returning(|a, b| a.len() + b.len())
            .once();

        assert_eq!(11, my_func_async(&d).await);
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Container(usize);

    #[test_case(1, 2 => Container(3); "should be able to add 1 and 2")]
    #[test_case(2, 3 => Container(5); "should be able to add 2 and 3")]
    fn add(x: usize, y: usize) -> Container {
        Container(x + y)
    }

    #[test_case(0 => matches None; "0 cannot be turned into a non zero usize")]
    #[test_case(1 => matches Some(_); "1 can be turned into a non zero usize")]
    fn create_non_zero_usize(u: usize) -> Option<NonZeroUsize> {
        NonZeroUsize::new(u)
    }
}
