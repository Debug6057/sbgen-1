// runs sbgen off the page's thread so big images don't freeze it
import init, * as sbgen from "./pkg/sbgen.js";

const ready = init();
// without this a missing or broken pkg only shows up as an error on each click
ready.catch((error) => self.postMessage({ loadError: error.message ?? String(error) }));

self.onmessage = async ({ data: { id, command, args } }) => {
  try {
    await ready;
    const result = sbgen[command](...args);
    // a plain string for emote, an Output (with getters) for the rest
    let reply;
    if (typeof result === "string") {
      reply = { content: result, description: "" };
    } else {
      reply = { content: result.content, description: result.description };
      result.free();
    }
    self.postMessage({ id, ...reply });
  } catch (error) {
    // a rust panic traps the instance for good, the page starts a new worker
    const crashed = error instanceof WebAssembly.RuntimeError;
    self.postMessage({
      id,
      crashed,
      error: crashed
        ? "The generator crashed (details are in the browser console) and was restarted."
        : (error.message ?? String(error)),
    });
  }
};
