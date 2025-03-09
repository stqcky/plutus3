use criterion::{Criterion, criterion_group, criterion_main};
use dotenvy_macro::dotenv;
use plutus_geth::db::GethDB;

fn benchmark(c: &mut Criterion) -> anyhow::Result<()> {
    let db = GethDB::new(dotenv!("CLIENT_DB"))?;

    c.bench_function("get_block_hash", |b| {
        b.iter(|| db.get_block_hash().unwrap())
    });

    c.bench_function("get_block_number", |b| {
        b.iter(|| db.get_block_number().unwrap())
    });

    c.bench_function("get_block_header", |b| {
        b.iter(|| db.get_block_header().unwrap())
    });

    Ok(())
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
