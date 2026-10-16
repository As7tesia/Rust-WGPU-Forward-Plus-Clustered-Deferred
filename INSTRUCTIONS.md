Forward+ and Clustered Deferred Shading (Rust + wgpu) - Instructions
==========================================================

**This is due Friday, October 16th at 11:59 PM.**

In this project, you will implement the Forward+ and Clustered Deferred shading methods as discussed in class. You are given a scene with the Sponza atrium model and a large number of point lights, as well as a GUI to toggle between the different rendering modes.

This version of the project is written in Rust with [wgpu](https://wgpu.rs/), an implementation of the WebGPU API that runs natively (Vulkan, DX12, Metal) and in the browser (WebGPU, through WebAssembly). The shaders are the same WGSL files as in the TypeScript version.

## Contents

- `src/` contains all the Rust code for this project:
  - `main.rs` is the entry point for both the desktop and the web build, and passes window events to the app
  - `app.rs` sets up the GPU, scene, camera, lights and GUI, and runs the frame loop
  - `renderer.rs` initializes wgpu and defines the `Renderer` trait and the layouts shared by all renderers
  - `renderers/` defines the different renderers in which you will implement Forward+ and Clustered Deferred shading
  - `stage/` includes camera controls, scene loading, and lights, where you will implement the clustering compute shader
  - `shaders.rs` loads and preprocesses the shaders
- `shaders/` contains the WGSL files that are compiled as shader programs at runtime
- `assets/` contains the Sponza Atrium model used in the test scene, as a single binary glTF (`.glb`) file
- `index.html` and `style.css` are the web page that [trunk](https://trunkrs.dev/) builds the web version into

## Running the code

Follow these steps to install and view the project:
- Clone this repository
- Install Rust with [rustup](https://rustup.rs/). The first build reads `rust-toolchain.toml` and installs the right toolchain and the `wasm32-unknown-unknown` target automatically.
  - On Windows, rustup also needs the Visual Studio C++ build tools.
- Desktop: run `cargo run` in the root directory of this project, which builds the project and opens it in a window
  - Use `cargo run --release` for performance measurements. Debug builds of your own code are much slower.
  - Shaders are embedded into the executable when it is built, so restart with `cargo run` after editing them
- Web: run `cargo install --locked trunk` once, then `trunk serve --open`, which will open the project in your browser
  - The project will automatically rebuild and reload when you edit any of the files
  - `trunk serve --release` builds an optimized version

Controls: hold a mouse button and move the mouse to look around, WASD to move, Q and E to move down and up. Hold Shift to move faster and Alt to move slower.

### Notes:

- The project will **not** work immediately after being cloned and set up as there are some tasks for you to complete to get it up and running (see [the project instructions](#part-1-implement-the-different-rendering-methods)). Until then it stops with a message pointing at the next TODO.
- The frame time in the top left corner is averaged over half a second. It is capped by your display's refresh rate (vsync). For measurements on desktop, change `wgpu::PresentMode::AutoVsync` to `AutoNoVsync` in `src/renderer.rs`.
- Browsers and GPUs (web build)
  - This project requires a WebGPU-capable browser. Ensure that you can see the Sponza scene being rendered using this [WebGPU test](https://toji.github.io/webgpu-test/).
    - Google Chrome seems to work best on all platforms.
    - Try [Google Chrome Canary](https://www.google.com/chrome/canary/) for the latest updates.
  - If you have problems running the starter code, use Chrome and make sure you have updated your browser and video drivers.
  - Remember to follow steps from [Project 0](https://github.com/CIS5650-Fall-2024/Project0-Getting-Started/blob/main/INSTRUCTION.md#part-23-project-instructions---webgpu) if needed.
- Ensure that your main GPU is the one being used. On desktop, the program prints the adapter name and backend when it starts. In the browser, check that `Adapter Info -> Description` on https://webgpureport.org/ is your main GPU. Often your low-powered GPU will be selected as default. To make a permanent switch, use your OS's GPU Settings to make the GPU default for your browser.

### Debugging Tools

Desktop:
- wgpu validates every call. A validation error stops the program with a message that names the failing call and the label of each object involved (`label: Some("naive pipeline")`), so label everything you create. Run from a terminal or from the debugger to see the message.
- Set `RUST_BACKTRACE=1` to get a stack trace with a panic, and `RUST_LOG` to change what gets logged (e.g. `RUST_LOG=info` shows wgpu's own info messages).
- Set `WGPU_BACKEND` to `vulkan` or `dx12` (or `metal` on macOS) to pick a backend.
- In VS Code, the CodeLLDB extension (or the C/C++ extension's debugger on Windows) can step through the Rust code; rust-analyzer adds "Run | Debug" buttons above `fn main`.
- [RenderDoc](https://renderdoc.org/) captures frames from the Vulkan and DX12 backends, showing every pass, bound resource and texture. NVIDIA Nsight Graphics and PIX (DX12) also work.

Web:
- You can use the standard console debugger in the browser’s developer tools to inspect and debug your code. Rust panics and `log` messages are printed to the console.
- Additionally, you can utilize the [WebGPU Inspector extension](https://github.com/brendan-duncan/webgpu_inspector?tab=readme-ov-file) to capture frames and inspect detailed GPU-related information. The tool is designed and implemented by Brendan Duncan.
  - Once you have installed the extension in Chrome, **Select More Tools / Developer Tools, or press F12 or Shift+CTRL+J (Option + ⌘ + J on MacOS)**. You can also right-click on the page and select Inspect. When the WebGPU Inspector extension is enabled in the extension manager, there will be a WebGPU Inspector tab.
  - If you have more question about this tool you can try to check the readme of the repo first.

### GitHub Pages setup (5 points)

Since this project uses WebGPU, it is easy to deploy it on the web for anyone to see. To set this up, do the following:
- Go to your repository's settings
- Go to the "Pages" tab
- Under "Build and Deployment", set "Source" to "GitHub Actions"

You will also need to go to the "Actions" tab in your repository and enable workflows there.

Once you've done those steps, any new commit to the `main` branch should automatically deploy to the URL `<username>.github.io/<repo_name>`. The workflow in `.github/workflows/static.yml` builds the web version with trunk.

## Requirements

**Ask on Ed Discussion for any clarifications.**

In this project, you are given code for:
- glTF scene loading
- Camera control
- Light movement compute shader
- Naive forward renderer
- Skeleton code for Forward+ and Clustered Deferred renderers
- Helpers

For editing the project, you will want to use [Visual Studio Code](https://code.visualstudio.com/) with the [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) extension. Once you've installed VSCode, you can open the root folder of this project using "File > Open Folder..." to start coding. You may also find [this extension](https://marketplace.visualstudio.com/items?itemName=PolyMeilex.wgsl) useful for highlighting WGSL syntax.

wgpu errors appear in the terminal on desktop and in your browser's developer console on the web (Ctrl + Shift + J for Chrome on Windows). Unlike some other graphics APIs, WebGPU error messages are often very helpful, especially if you've labeled your various pipeline components with meaningful names. Be sure to check them whenever something isn't working correctly.

### Part 1: Implement the different rendering methods 

To start off, the naive renderer is missing a camera view projection matrix buffer, and your job is to fill in the missing parts. This will expose you to various parts of the codebase and will hopefully help you understand the general layout of the WebGPU rendering pipeline.

#### 1) Naive (20 points)

1.1) Create and write to the buffer
- You first need to create the buffer in `src/stage/camera.rs` and write the view projection matrix to it
- Then, you need to upload the buffer to the GPU
- Look for comments containing `TODO-1.1` for details

1.2) Use the buffer in a bind group and a render pass
- You then need to use the buffer in the naive renderer's layouts and pipeline in `src/renderers/naive.rs`
- Look for comments containing `TODO-1.2` for details

1.3) Update the shaders accordingly
- Lastly, you need to update the naive renderer shaders to actually use the new buffer
- Look for comments containing `TODO-1.3` for details

Until Part 1 is done, `cargo build` shows a few warnings (unused variables, unreachable code) at the `TODO-1` sites. They go away as you complete it.

Then, based on the discussions in lecture and recitation, you are expected to implement the Forward+ and Clustered Deferred rendering methods and analyze their results. Here is a summary of both methods:

#### 2) Forward+ (50 points)

  - Build a data structure to keep track of how many lights are in each cluster and what their indices are
  - Render each fragment using only the lights that overlap its cluster
  - Look for comments containing `TODO-2` for details

When adding new buffers, especially if they contain new structs, their alignment might be different than what you expect. Be sure to check your structs' alignment using [this online calculator](https://webgpufundamentals.org/webgpu/lessons/resources/wgsl-offset-computer.html#) and match the memory layout on the host. On the Rust side, use `#[repr(C)]` structs that derive bytemuck's `Pod` and `Zeroable` (see `Light` in `src/stage/lights.rs`), with explicit padding fields where WGSL pads, and a `const _: () = assert!(size_of::<T>() == N);` check.

#### 3) Clustered Deferred (15 points)

  - Reuse the clustering logic from Forward+
  - Store vertex attributes in a G-buffer
  - Read from the G-buffer in a separate fullscreen pass to produce final output
  - Look for comments containing `TODO-3` for details

### Part 2: Extra Credit: Effects and Optimizations (maximum 25 points)

For full credit, you must show a good optimization effort and record the performance of each version you test.

#### Extra Credit: Post Processing (5 points)

Implement one of the following post-processing effects:
- Bloom using post-process blur (box or Gaussian)
- Toon shading (with ramp shading + simple depth-edge detection for outlines)

For full credit, you must create a new compute pass (not a fullscreen rendering pass) for post-processing.

#### Extra Credit: G-buffer Optimization (10 points)

Use a single compute pass to replace the vertex + fragment shader fullscreen rendering pass you are provided in the base code. (+5)

Optimize the G-buffer used for the Clustered Deferred renderer. In particular, aim to reduce the amount of textures and the size of per-pixel data. You will receive full points if your G-buffer uses only one color output image and each pixel stores additional data less than or equal to one `vec4f`. (+5) 

Here are some ideas to get you started:

- Pack values together into `vec4`s
  - Use `vec4f` or `vec4u` depending on how you pack your data
- Use 2-component normals
  - For even more compression, look into octahedron normal encoding, which can even be packed into one `u32`
- Quantize values by packing them into smaller data types
  - You may find functions like [`pack2x16snorm`](https://www.w3.org/TR/WGSL/#pack2x16snorm-builtin) helpful
- Reduce the number of properties passed via the G-buffer
  - For example, instead of storing world position in a texture, reconstruct it using camera matrices and depth

#### Extra Credit: Visibility Buffer (15 points)

For devices with limited GPU bandwidth, we can try to further reduce the memory footprint of the geometry pass. This can be done by using a single channel `u32` buffer for shading. Here are some hints to do that:

1. Rewrite the current G-buffer code to output ObjectID and TriangleID; the format can be something like (`(ObjectID << offset) + TriangleID`)
1. In the shading stage, bind the triangles index buffer and vertex buffer as two storage buffers and load vertex attributes according to ObjectID and TriangleID
1. Use the current pixel's position and depth to reconstruct its world position, then calculate the barycentric coordinates using the world positions of the current pixel and the three triangle vertices
1. Interpolate vertex attributes
1. Perform shading according to ObjectID (you don't need to do mipmapping when sampling texture here to receive full credits)

Note that if you want to implement this in addition to the G-buffer optimizations, in order to receive full credit for both, you will need some method of switching between the two pipelines.

For more reference, please refer to the following materials:

- [The Visibility Buffer: A Cache-Friendly Approach to Deferred Shading (JCGT)](https://jcgt.org/published/0002/02/04/)
- [Visibility Buffer Rendering with Material Graphs – Filmic Worlds](http://filmicworlds.com/blog/visibility-buffer-rendering-with-material-graphs/)

#### Extra Credit: Render Bundles (5 points)

Use [render bundles](https://toji.dev/webgpu-best-practices/render-bundles.html) to reduce the overhead of host-side draw calls (in wgpu: `Device::create_render_bundle_encoder` and `RenderPass::execute_bundles`). Make sure you provide a performance analysis showing the effect of this change.

#### Extra Credit: Depth Prepass for Forward+ (15)

Add a depth-only prepass to the Forward+ renderer and reuse its results to accelerate both light clustering and final shading. This eliminates shading overdraw in the final lighting pass (each visible pixel is lit exactly once) and lets you narrow the set of clusters that need to be considered.

Implement the prepass itself, plus one or both of the optimizations below:

1. **Overdraw-free shading (+5)**
    - Render the geometry to depth only, then reuse that depth buffer in the final lighting pass so that only visible fragments are shaded.
    - Think carefully about the depth comparison and write settings each pass needs.

2. **Depth-based cluster culling (+10)**
    - In a separate fullscreen-quad pass (after the depth prepass but before light clustering), use the depth buffer to identify and mark which clusters contain geometry and are not occluded.
    - In the light clustering step, only consider those identified clusters (you can either simply skip inactive clusters, or -- potentially even better -- run a stream compaction step first, and then dispatch fewer threads for the light clustering shader. Make sure to indicate your approach in the performance analysis).

Your performance analysis should include the results of these optimizations in its discussion. Possible considerations: how the optimization scales with scene depth complexity (overdraw, empty clusters), how it scales with the number of lights, whether stream compaction is worth it, etc.

## Performance Analysis (10 points)

Compare your implementations of Forward+ and Clustered Deferred shading and analyze their differences.
- Is one of them faster?
- Is one of them better at certain types of workloads?
- What are the benefits and tradeoffs of using one over the other?
- For any differences in performance, briefly explain what may be causing the difference.

Optimize your Rust and/or WGSL code. For desktop builds, measure with `cargo run --release` and vsync off (see [Notes](#notes)); GPU profilers such as Nsight Graphics or RenderDoc show the cost of each pass. For the web build, Chrome's profiling tools are useful. For each change that improves performance, show the before and after render times.

If your Forward+ or Clustered Deferred renderer is running much slower than expected, make sure you are not making copies of large structs/arrays in shader code. You can use [pointers](https://google.github.io/tour-of-wgsl/types/pointers/using/) in WGSL to avoid this issue.

For each new effect feature (required or extra), please provide the following analysis:
  - Concise overview and explanation of the feature.
  - Performance change due to adding the feature.
  - If applicable, how do parameters (such as number of lights, number of tiles, etc.) affect performance? Show data with graphs.
    - Show timing in milliseconds, not FPS.
  - If you did something to accelerate the feature, what did you do and why?
  - How might this feature be optimized beyond your current implementation?

For each performance feature (required or extra), please provide:
  - Concise overview and explanation of the feature.
  - Detailed performance improvement analysis of adding the feature.
    - What is the best case scenario for your performance improvement? What is the worst? Explain briefly.
    - Are there tradeoffs to this performance feature? Explain briefly.
    - How do parameters (such as number of lights, number of tiles, etc.) affect performance? Show data with graphs.
      - Show timing in milliseconds, not FPS.
    - Show debug views when possible.
      - If the debug view correlates with performance, explain how.

## Base Code Walkthrough

In general, you can search for comments containing "CHECKITOUT" to see the most important/useful parts of the base code.

- `src/main.rs` creates the window and the event loop, and hands window and mouse events to the app. It also contains the parts that differ between desktop and web (`#[cfg(target_arch = "wasm32")]`). You likely won't have to change this file.
- `src/app.rs` initializes wgpu and the various shared entities (lights, camera, scene, etc.), the GUI and the renderer itself, and runs the frame loop: it updates the camera and lights, then calls the active renderer's `draw`. You likely won't have to change this file unless you want to edit the GUI.
- `src/stage/` contains all the code that deals with the information in a stage.
  - `src/stage/camera.rs` contains camera controls. You will need to add new uniform buffers here for certain camera matrices for the Forward+ and Deferred Clustered renderers.
  - `src/stage/lights.rs` controls the lights' positions and color. This is where you will call the light clustering compute shader.
  - `src/stage/scene.rs` loads glTF scene files into a flat list of draws sorted by material. You won't need to edit this file unless you want to load scenes other than the provided Sponza scene.
- `src/renderer.rs` initializes wgpu (`Gpu`) and defines the `Renderer` trait that all renderers implement, along with the vertex layout and the bind group layouts they share. The bulk of the logic you need to write will go in the renderers, so you won't need to edit this file.
- `src/renderers/` contains the implementations of `Renderer`. The file names are self-explanatory. This is where most of the new host-side logic will be written. `App` owns the scene, camera and lights and passes them to `draw` every frame; a renderer is recreated when you switch modes in the GUI, and `on_resize` is called when the window changes size.
- `src/shaders.rs` loads and preprocesses the WGSL files in `shaders/`. You can add constants here that can be directly referenced in shaders as `${name}`, similar to preprocessor defines in C++.
- `shaders/` contains all the WGSL shaders. `shaders/common.wgsl` contains some shader utility functions that are prepended to all shaders. Code used by multiple shaders should go here if possible.
- `src/math_util.rs` contains a small random number generator. Feel free to add more math helper functions; [glam](https://docs.rs/glam) provides the vector and matrix types.

## README

Replace the contents of `README.md` with the following:
- A brief description of your project and the specific features you implemented
- At least one screenshot of your project running
- A 30+ second video/gif of your project running showing all features (even though your demo can be seen online, it may not run on all computers, while a video will work everywhere)
- A link to your project's live website (see [GitHub Pages setup](#github-pages-setup-5-points))
- Performance analysis (see [above](#performance-analysis-10-points))

## Submit

Open a GitHub pull request so that we can see that you have finished. The title should be "Project 4: YOUR NAME". The template of the comment section of your pull request is attached below, you can do some copy and paste:

- Repo Link
- Briefly mention features that you've completed, especially those bells and whistles you want to highlight:
  - Feature 0
  - Feature 1
  - ...
- Feedback on the project itself, if any.

### Third-Party Code Policy

- Use of any third-party code must be approved by asking on Ed Discussion.
- If it is approved, all students are welcome to use it. Generally, we approve use of third-party code that is not a core part of the project. For example, for the path tracer, we would approve using a third-party library for loading models, but would not approve copying and pasting a CUDA function for doing refraction.
- Third-party code **MUST** be credited in README.md.
- Using third-party code without its approval, including using another student's code, is an academic integrity violation, and will, at minimum, result in you receiving an F for the semester.
