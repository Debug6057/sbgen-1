// runs sbgen off the page's thread so big images don't freeze it
import init, * as sbgen from "./pkg/sbgen.js";

const ready = init();

self.onmessage = async ({ data: { id, command, args } }) => {
  try {
    await ready;
    const result = sbgen[command](...args);
    // a plain string for emote, an Output (with getters) for the rest
    const reply =
      typeof result === "string"
        ? { content: result, description: "" }
        : { content: result.content, description: result.description };
    self.postMessage({ id, ...reply });
  } catch (error) {
    self.postMessage({ id, error: error.message ?? String(error) });
  }
};
