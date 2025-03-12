
// self.onmessage = async function(e) {
//     let track = e.data.track;
//     console.log(e.data);
//     const trackProcessor = new MediaStreamTrackProcessor(track);
//     const reader = trackProcessor.readable.getReader();
//     while (true) {
//         const result = await reader.read();
//         if (result.done) break;
//         const frame = result.value;
//         self.postMessage(frame);
//     }
// }
self.onmessage = async ({data: {track}}) => {
    console.log(track);
    // const vtg = new VideoTrackGenerator();
    // self.postMessage({track: vtg.track}, [vtg.track]);
    const trackProcessor = new MediaStreamTrackProcessor({track});
    const reader = trackProcessor.readable.getReader();
    while (true) {
        const result = await reader.read();
        if (result.done) break;
        const frame = result.value;
        self.postMessage(frame);
    }
    // await mstp.readable.pipeThrough(new TransformStream({transform})).pipeTo(vtg.writable);
  };