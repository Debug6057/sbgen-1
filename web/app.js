const SPECIES = [
  "apex",
  "avian",
  "floran",
  "glitch",
  "human",
  "hylotl",
  "novakid",
];

// fields: file (optional unless required), select, check
const COMMANDS = {
  sprite: {
    title: "Sprite",
    help: "A single sprite of up to 256x256 pixels as directives, or an item that wears them.",
    fields: [
      { name: "image", label: "Image", type: "file", required: true },
      {
        name: "type",
        label: "Output",
        type: "select",
        options: ["raw", "hat", "object", "floating-object"],
      },
    ],
    json: (v) => v.type !== "raw",
    run: (v) => ["sprite", [v.image, v.type]],
  },
  chest: {
    title: "Shirt",
    help: "A custom shirt item. Give a female chest, a male chest or both, and optionally sleeves.",
    fields: [
      { name: "chestFemale", label: "Female chest", type: "file" },
      { name: "chestMale", label: "Male chest", type: "file" },
      { name: "frontSleeve", label: "Front sleeve", type: "file" },
      { name: "backSleeve", label: "Back sleeve", type: "file" },
    ],
    need: (v) =>
      v.chestFemale.length || v.chestMale.length
        ? null
        : "Give at least one of the chests.",
    json: () => true,
    run: (v) => [
      "shirt",
      [v.chestFemale, v.chestMale, v.frontSleeve, v.backSleeve],
    ],
  },
  pants: {
    title: "Pants",
    help: "A custom pants item.",
    fields: [
      { name: "pants", label: "Pants", type: "file", required: true },
      {
        name: "hideBody",
        label: "Hide the body under the pants",
        type: "check",
      },
    ],
    json: () => true,
    run: (v) => ["pants", [v.pants, v.hideBody]],
  },
  back: {
    title: "Back",
    help: "A custom back item.",
    fields: [{ name: "back", label: "Back", type: "file", required: true }],
    json: () => true,
    run: (v) => ["back", [v.back]],
  },
  humanoid: {
    title: "Humanoid",
    help: "Custom body directives for the identity of a character.",
    fields: [
      {
        name: "bodyType",
        label: "Body type",
        type: "select",
        options: ["male", "female"],
      },
      {
        name: "species",
        label: "Species",
        type: "select",
        options: [...SPECIES, "other"],
      },
      { name: "body", label: "Body", type: "file" },
      { name: "head", label: "Head", type: "file" },
      { name: "frontArm", label: "Front arm", type: "file" },
      { name: "backArm", label: "Back arm", type: "file" },
    ],
    need: (v) =>
      v.body.length || v.head.length || v.frontArm.length || v.backArm.length
        ? null
        : "Give at least one image.",
    run: (v) => [
      "humanoid",
      [v.bodyType, v.species, v.body, v.head, v.frontArm, v.backArm],
    ],
  },
  emote: {
    title: "Emotes",
    help: "The emoteDirectives of a 387x344 emote sheet.",
    fields: [
      { name: "sheet", label: "Emote sheet", type: "file", required: true },
      { name: "species", label: "Species", type: "select", options: SPECIES },
    ],
    run: (v) => ["emote", [v.sheet, v.species]],
  },
};

let worker;
let loadError = null;
let nextId = 0;
const pending = new Map();
const failAll = (message) => {
  for (const { reject } of pending.values()) reject(new Error(message));
  pending.clear();
};
// a panic leaves the wasm instance unusable, so a crashed worker is replaced
function startWorker() {
  worker?.terminate();
  worker = new Worker(new URL("./worker.js", import.meta.url), {
    type: "module",
  });
  worker.onmessage = ({ data }) => {
    if (data.loadError !== undefined) {
      loadError = `The generator failed to load: ${data.loadError}`;
      showLoadError();
      return;
    }
    const { resolve, reject } = pending.get(data.id);
    pending.delete(data.id);
    if (data.crashed) startWorker();
    data.error === undefined ? resolve(data) : reject(new Error(data.error));
  };
  worker.onerror = (event) => {
    failAll(event.message || "the worker failed");
    startWorker();
  };
}
const call = (command, args) =>
  new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    worker.postMessage({ id, command, args });
  });

const $ = (id) => document.getElementById(id);
const tabs = $("tabs");
const form = $("form");
const help = $("help");
const result = $("result");
const output = $("output");
const description = $("description");
const errorBox = $("error");
const status = $("status");
let current = "sprite";
let downloadExtension = "txt";

function showLoadError() {
  if (!loadError) return;
  errorBox.textContent = loadError;
  errorBox.hidden = false;
}

function show(key) {
  current = key;
  const command = COMMANDS[key];
  for (const button of tabs.children)
    button.setAttribute("aria-selected", String(button.dataset.key === key));
  help.textContent = command.help;
  form.replaceChildren();
  for (const field of command.fields) {
    const label = document.createElement("label");
    const text = document.createElement("span");
    text.textContent = field.label + (field.required ? " *" : "");
    let input;
    if (field.type === "select") {
      input = document.createElement("select");
      for (const option of field.options) input.add(new Option(option, option));
    } else {
      input = document.createElement("input");
      input.type = field.type === "file" ? "file" : "checkbox";
      if (field.type === "file") input.accept = "image/png";
    }
    input.name = field.name;
    label.className = field.type === "check" ? "check" : "";
    label.append(...(field.type === "check" ? [input, text] : [text, input]));
    form.append(label);
  }
  const submit = document.createElement("button");
  submit.type = "submit";
  submit.textContent = "Generate";
  form.append(submit);
  result.hidden = true;
  errorBox.hidden = true;
  showLoadError();
}

async function values(command) {
  const out = {};
  for (const field of command.fields) {
    const input = form.elements[field.name];
    if (field.type === "file") {
      const file = input.files[0];
      out[field.name] = file
        ? new Uint8Array(await file.arrayBuffer())
        : new Uint8Array();
    } else {
      out[field.name] = field.type === "check" ? input.checked : input.value;
    }
  }
  return out;
}

form.onsubmit = async (event) => {
  event.preventDefault();
  const command = COMMANDS[current];
  errorBox.hidden = true;
  result.hidden = true;
  try {
    if (loadError) throw new Error(loadError);
    const v = await values(command);
    const missing = command.fields.find(
      (field) => field.required && !v[field.name].length,
    );
    if (missing) throw new Error(`${missing.label} is needed.`);
    const problem = command.need?.(v);
    if (problem) throw new Error(problem);
    status.textContent = "Generating...";
    const [name, args] = command.run(v);
    const reply = await call(name, args);
    output.value = reply.content;
    downloadExtension = command.json?.(v) ? "json" : "txt";
    description.textContent =
      reply.description || `${reply.content.length} chars`;
    result.hidden = false;
  } catch (error) {
    errorBox.textContent = error.message;
    errorBox.hidden = false;
  } finally {
    status.textContent = "";
  }
};

$("copy").onclick = async () => {
  try {
    await navigator.clipboard.writeText(output.value);
    $("copy").textContent = "Copied";
  } catch {
    output.select();
    $("copy").textContent = "Select and copy";
  }
  setTimeout(() => ($("copy").textContent = "Copy"), 1500);
};

$("download").onclick = () => {
  const link = document.createElement("a");
  link.href = URL.createObjectURL(
    new Blob([output.value], { type: "text/plain" }),
  );
  link.download = `${current}.${downloadExtension}`;
  link.click();
  URL.revokeObjectURL(link.href);
};

startWorker();
for (const [key, command] of Object.entries(COMMANDS)) {
  const button = document.createElement("button");
  button.type = "button";
  button.role = "tab";
  button.dataset.key = key;
  button.textContent = command.title;
  button.onclick = () => show(key);
  tabs.append(button);
}
show(current);
