# rig-experimental
A companion crate for [`rig`](https://github.com/0xPlaygrounds/rig) that offers some helpful extras you may need for your agentic AI pipelines.

This is mostly an experimental crate, so expect to see things break.

## Current features
- Semantic Routing: set up a semantic router with `SemanticRouter`, then add your vector store of choice (that implements `rig::vector_store::VectorStoreIndex`) and start adding some routes and agents!
- Autonomous agent abstraction
- Extra providers that integrate directly into `rig`:
  - Candle
  - OpenAI Realtime API
  - ElevenLabs (currently TTS only; more modes incoming)
