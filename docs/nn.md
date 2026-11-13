# Neural Network Models (§6.4)

RCA's NN stack is built from composable primitives, not an opaque Model object. Every layer is a builtin; every model is a user function that combines them.

## Primitives

| Primitive | Signature |
|---|---|
| linear | linear(x, w, b) |
| relu | relu(x) |
| sigmoid | sigmoid(x) |
| tanh | tanh(x) |
| softmax | softmax(x) |
| conv2d | conv2d(x, k) |
| mse | mse(pred, target) |
| sgd | sgd(w, grad, lr) |

## Autodiff primitives

| Primitive | Signature |
|---|---|
| track | track(t) — mark a leaf |
| grad | grad(y, x) — dy/dx |

## Training pattern

```rca
let x = Tensor([[1.0, 2.0]])
let y = Tensor([[1.0]])
mut w1 = Tensor([[0.1, 0.2], [0.3, 0.4]])
mut b1 = Tensor([0.0, 0.0])
mut w2 = Tensor([[0.5], [0.6]])
mut b2 = Tensor([0.0])
for i in 0..3:
    let w1t = track(w1)
    let h = relu(linear(x, w1t, track(b1)))
    let pred = linear(h, track(w2), track(b2))
    let loss = mse(pred, y)
    w1 = sgd(w1, grad(loss, w1t), 0.05)
    print(loss)
```
## Examples
- examples/train_linear.rca — single-layer regression
- examples/mlp_train.rca — two-layer MLP
