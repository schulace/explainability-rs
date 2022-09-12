use crate::Operation;
use crate::OperationType;
use crate::Operator;
use typed_arena::Arena;

#[test]
fn test_sum_reasons() {
    fn within_point1(val: f32, target: f32) -> bool {
        target - 0.1 < val && val < target + 0.1
    }
    let arena = Arena::new();
    let a = Operation::new_with_reason(1.0, "a", &arena);
    let b = Operation::new(2.0, &arena);
    use OperationType::*;
    let a_plus_b = a + b;
    assert!(matches!(a_plus_b.op, Sum { .. }));
    assert!(matches!(a_plus_b.reason, None));
    let a_plus_b = a + (b, "b");
    assert!(matches!(&a_plus_b.reason, Some(r) if r == "b"));
    let continuing_sum = a_plus_b + a;
    assert!(
        matches!(&continuing_sum.op, Sum { value, history } if history.len() == 3 && *value < 4.1)
    );
    assert!(matches!(&continuing_sum.reason, Some(r) if r == "b"));
    let c = Operation::new_with_reason(3.0, "c", &arena);
    let continuing_sum = continuing_sum + c;
    assert!(matches!(
        continuing_sum,
        Operation {
            op: Sum { history, value, .. },
            reason: Some(r),
            ..
        } if r == "b" && matches!(history[..], [
                      Operation { op: Source{ .. }, reason: Some(r1), .. },
                      Operation { op: Source{ .. }, reason: None, .. },
                      Operation { op: Source{ .. }, reason: Some(r2), .. },
                      Operation { op: Source{ .. }, reason: Some(r3), .. }
        ] if r1 == "a" && r2 == "a" && r3 == "c") && within_point1(*value, 7.)
    ));
    dbg!(web_graph(continuing_sum));
    println!("{}", continuing_sum.as_json());
}

#[allow(dead_code)]
fn write_graph<'a>(op: &'a Operation<'a>, file: impl Into<Option<&'static str>>) {
    let filename = file.into().unwrap_or("output.dot");
    let mut file = std::fs::File::create(filename).unwrap();
    let graph = op.as_graphviz();
    use std::io::Write;
    file.write_all(graph.as_bytes()).unwrap();
}

fn web_graph<'a>(op: &'a Operation<'a>) -> String {
    let graph = op.as_graphviz();
    let query = urlencoding::encode(&graph);
    format!("https://dreampuf.github.io/GraphvizOnline/#{query}")
}

#[derive(Debug)]
struct Sqrt;
impl Operator for Sqrt {
    fn symbol(&self) -> &'static str {
        " sqrt "
    }
    fn operate<'a>(&'a self, ops: &[&'a Operation<'a>]) -> &'a Operation {
        let operand = ops[0];
        operand._allocator.alloc(Operation {
            op: OperationType::Other {
                value: f32::sqrt(operand.op.value()),
                op: self,
                history: vec![operand],
            },
            reason: None,
            _allocator: operand._allocator,
        })
    }
}

#[test]
fn graph_render() {
    let arena = Arena::new();
    let a = Operation::new_with_reason(1.0, "a", &arena);
    let b = Operation::new(2.0, &arena);
    let a_plus_b = a + (b, "b");
    let continuing_sum = a_plus_b + a;
    let c = Operation::new_with_reason(3.0, "c", &arena);
    let continuing_sum = continuing_sum + c;
    println!("{}", web_graph(continuing_sum));
}

fn fibonacci<'a>(steps: u32, alloc: &'a Arena<Operation<'a>>) -> &'a Operation<'a> {
    assert!(steps > 0);
    let a = Operation::new_with_reason(0.0, "definitional", alloc);
    if steps == 1 {
        return a;
    }
    let b = Operation::new_with_reason(1.0, "definitional", alloc);
    if steps == 2 {
        return b;
    }
    // a + b
    fibonacci(steps - 1, alloc) + (fibonacci(steps - 2, alloc), format!("fib({steps})"))
}

#[test]
fn test_fib() {
    let alloc = Arena::new();
    let fib5 = fibonacci(5, &alloc);
    dbg!(web_graph(fib5));
}

fn newton_sqrt<'a>(
    target: &'a Operation<'a>,
    iters: u32,
    alloc: &'a Arena<Operation<'a>>,
) -> &'a Operation<'a> {
    let mut guess = target;
    let two = Operation::new_with_reason(2.0, "constant", alloc);
    for n in 0..iters {
        guess = (guess + (target / guess)) / (two, format!("iteration {n} approx"));
    }
    guess
}

#[test]
fn approx_sqrt() {
    let alloc = Arena::new();
    let target = Operation::new_with_reason(42., "initial", &alloc);
    let sqrt = Sqrt;
    let sqrt: &dyn Operator = &sqrt;
    let actual_sqrt = sqrt.operate(&[target]);
    let guess = newton_sqrt(target, 6, &alloc);
    let square_root_approx_error = guess - (actual_sqrt, "error");
    dbg!(web_graph(square_root_approx_error));
}

#[test]
fn chained_add() {
    let alloc = Arena::new();
    let chain_sum = (1..=10)
        .map(|n| Operation::new(n as f32, &alloc))
        .fold(Operation::new(0., &alloc), |acc, x| acc + x);
    dbg!(web_graph(chain_sum));
}

#[test]
fn non_commutative() {
    let alloc = Arena::new();
    let (op, _) = Operation::make_ctors(&alloc);
    let a = op(6.) / op(3.);
    assert_eq!(a.value(), 6. / 3.);
    let c = a / op(3.);
    assert_eq!(c.value(), 6. / 3. / 3.);
}
