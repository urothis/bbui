# NodeGraph GPU review captures

These Dark and Light PNGs show the four-node, five-link scene from the public NodeGraph example at 1280×820. They were captured from a Bevy offscreen image target with Mesa llvmpipe after rendering completed. They are review images, not byte-for-byte test baselines; the GPU test checks tolerant color and geometry regions, including socket circles, link colors, node text containment, theme surfaces, and a known empty stripe between links.

Regenerate them with:

```sh
BUI_NODE_GRAPH_CAPTURE_DIR="$PWD/docs/visuals" cargo test -p bui --test node_graph_gpu_readback --locked
```

The Linux GPU CI job also uploads each run's captures as the `node-graph-gpu-review-captures` artifact.

- [Dark capture](node_graph_example_dark.png)
- [Light capture](node_graph_example_light.png)
