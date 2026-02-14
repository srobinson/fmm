const https = require("https");
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const REPO = "srobinson/fmm";
const VERSION = require("../package.json").version;
const TAG = `v${VERSION}`;

const TARGETS = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "win32-x64": "x86_64-pc-windows-msvc",
};

const key = `${process.platform}-${process.arch}`;
const target = TARGETS[key];

if (!target) {
  console.warn(`fmm: unsupported platform ${key}, skipping binary download`);
  process.exit(0);
}

const isWindows = process.platform === "win32";
const ext = isWindows ? ".zip" : ".tar.gz";
const url = `https://github.com/${REPO}/releases/download/${TAG}/fmm-${target}${ext}`;
const dest = path.join(__dirname, isWindows ? "fmm.exe" : "fmm");

function download(url, cb) {
  https.get(url, (res) => {
    if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
      return download(res.headers.location, cb);
    }
    if (res.statusCode !== 200) {
      console.warn(`fmm: download failed (${res.statusCode}) from ${url}`);
      console.warn(`fmm: install manually from https://github.com/${REPO}/releases`);
      return process.exit(0);
    }
    cb(res);
  }).on("error", (e) => {
    console.warn(`fmm: download error: ${e.message}`);
    process.exit(0);
  });
}

download(url, (stream) => {
  if (isWindows) {
    const tmp = path.join(__dirname, `fmm-${target}.zip`);
    const out = fs.createWriteStream(tmp);
    stream.pipe(out);
    out.on("finish", () => {
      try {
        execSync(`powershell -Command "Expand-Archive -Path '${tmp}' -DestinationPath '${__dirname}' -Force"`, { stdio: "ignore" });
        fs.unlinkSync(tmp);
      } catch (e) {
        console.warn("fmm: failed to extract windows archive");
        process.exit(0);
      }
    });
  } else {
    const tar = require("child_process").spawn("tar", ["xzf", "-", "-C", __dirname]);
    stream.pipe(tar.stdin);
    tar.on("close", (code) => {
      if (code !== 0) {
        console.warn("fmm: tar extraction failed");
        process.exit(0);
      }
      fs.chmodSync(dest, 0o755);
    });
  }
});
