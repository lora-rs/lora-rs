# Benchmarks

Ran on Intel i7-8550U CPU @ 1.80GHz with 16GB RAM running Ubuntu 18.04.

* Benchmarks [brocaar/lorawan][4] (the code for the benchmarks can be found
  [here][3], results were obtained by running `go test -bench . -benchtime=5s`,
  `go1.13.1`)

```bash
pkg: github.com/brocaar/lorawan
BenchmarkDecode-8                  40410            150498 ns/op
BenchmarkValidateMic-8              2959           2026736 ns/op
BenchmarkDecrypt-8                  9390            648402 ns/op
```

* Benchmarks rust-lorawan (the code is inside `benches/lorawan.rs`, results are
  obtained running `cargo bench --workspace`, `rustc 1.43.0`)

```bash
  Running target/release/deps/lorawan-32e80b41705c7d41
Gnuplot not found, using plotters backend

data_payload_headers_parsing
      time:   [30.354 ns 30.430 ns 30.497 ns]
      change: [-5.5657% -5.1359% -4.7052%] (p = 0.00 < 0.05)
      Performance has improved.
Found 1 outliers among 100 measurements (1.00%)
  1 (1.00%) high mild

Approximate memory usage per iteration: 1 from 303847227

data_payload_mic_validation
      time:   [2.2334 us 2.2388 us 2.2476 us]
      change: [-3.7708% -3.3970% -2.8941%] (p = 0.00 < 0.05)
      Performance has improved.
Found 20 outliers among 100 measurements (20.00%)
  2 (2.00%) low severe
  5 (5.00%) low mild
  2 (2.00%) high mild
  11 (11.00%) high severe

Approximate memory usage per iteration: 114 from 4349451

data_payload_decrypt
      time:   [1.1179 us 1.1186 us 1.1193 us]
      change: [-0.8167% -0.4650% -0.1514%] (p = 0.00 < 0.05)
      Change within noise threshold.
Found 8 outliers among 100 measurements (8.00%)
  2 (2.00%) low severe
  2 (2.00%) low mild
  3 (3.00%) high mild
  1 (1.00%) high severe

Approximate memory usage per iteration: 57 from 8668603
```

[3]: https://gist.github.com/ivajloip/d63981e4caddaa68bd0b9c2390f4af90
[4]: https://github.com/brocaar/lorawan/commit/6095d473cf605ce4da4584ae2b570bca8e1259ff
