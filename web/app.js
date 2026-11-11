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
    title: "Generic Sprite",
    help: "A single sprite of up to 256x256 pixels as directives, or an item that wears them.",
    fields: [
      { name: "image", label: "Image", type: "file", required: true },
      {
        name: "type",
        label: "Output Format",
        type: "select",
        options: ["raw", "hat", "object", "floating-object"],
      },
    ],
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
    run: (v) => ["pants", [v.pants, v.hideBody]],
  },
  back: {
    title: "Back",
    help: "A custom back item.",
    fields: [{ name: "back", label: "Back", type: "file", required: true }],
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

const worker = new Worker(new URL("./worker.js", import.meta.url), {
  type: "module",
});
let nextId = 0;
const pending = new Map();
worker.onmessage = ({ data }) => {
  const { resolve, reject } = pending.get(data.id);
  pending.delete(data.id);
  data.error === undefined ? resolve(data) : reject(new Error(data.error));
};
worker.onerror = (event) => {
  for (const { reject } of pending.values())
    reject(new Error(event.message || "the worker failed"));
  pending.clear();
};
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

function show(key) {
  current = key;
  const command = COMMANDS[key];

  for (const button of tabs.children)
    button.setAttribute("aria-selected", String(button.dataset.key === key));

  help.textContent = command.help;
  form.replaceChildren();

  form.append(help);
  form.append(document.createElement("hr"));

  for (const field of command.fields) {
    const label = document.createElement("label");
    const text = document.createElement("span");

    text.innerHTML = field.label + (field.required ? '<label class="requiredIcon" title="Required">*</label>' : "");

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
    label.className = field.type === "check" ? "check" : "file";
    label.append(...(field.type === "check" ? [input, text] : [text, input]));
    form.append(label);
  }
  
  const submit = document.createElement("button");
  submit.type = "submit";
  submit.className = "submit";
  submit.textContent = "Generate";
  form.append(submit);

  result.hidden = true;
  errorBox.hidden = true;
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

function init() {
  form.onsubmit = async (event) => {
    event.preventDefault();
    const command = COMMANDS[current];
    errorBox.hidden = true;
    result.hidden = true;
    try {
      const v = await values(command);
      const missing = command.fields.find(
        (field) => field.required && !v[field.name].length,
      );
      if (missing) throw new Error(`${missing.label} is needed.`);
      const problem = command.need?.(v);
      if (problem) throw new Error(problem);

      status.textContent = "Generating...";
      status.style.display = "unset";

      const [name, args] = command.run(v);
      const reply = await call(name, args);
      output.value = reply.content;
      description.textContent =
        reply.description || `${reply.content.length} chars`;
      result.hidden = false;
    } catch (error) {
      errorBox.textContent = error.message;
      errorBox.hidden = false;
    } finally {
      status.textContent = "";
      status.style.display = "none";
    }
  };

  $("copy").onclick = async () => {
    try {
      await navigator.clipboard.writeText(output.value);
      $("copy").textContent = "Copied!";
    } catch {
      output.select();
      $("copy").textContent = "Select and copy";
    }
    setTimeout(() => ($("copy").textContent = "Copy"), 1500);
  };

  $("download").onclick = () => {
    const link = document.createElement("a");
    const json = output.value.trimStart().startsWith("{");
    link.href = URL.createObjectURL(
      new Blob([output.value], { type: "text/plain" }),
    );
    link.download = `${current}.${json ? "json" : "txt"}`;
    link.click();
    URL.revokeObjectURL(link.href);
  };

  const modeSelector = document.createElement("select");
  for (const [key, command] of Object.entries(COMMANDS)) {
    modeSelector.appendChild(new Option(command.title, key));
  }
  modeSelector.addEventListener("change", function() {show(this.value)})
  tabs.append(modeSelector)

  show(current);
}

init();