// Picks the right download for the visitor's OS, links installers from the latest GitHub release,
// and wires the install tabs and copy buttons. Works without the API (links fall back to Releases).
(function () {
  var REPO = "Lautaro005/opencue";
  var SITE = "https://lautaro005.github.io/opencue";
  var CMD = {
    mac: "curl -fsSL " + SITE + "/install.sh | sh",
    linux: "curl -fsSL " + SITE + "/install.sh | sh",
    windows: "irm " + SITE + "/install.ps1 | iex",
  };
  var NAMES = { mac: "macOS", windows: "Windows", linux: "Linux" };
  var PATTERNS = {
    dmg: /\.dmg$/i,
    exe: /x64-setup\.exe$/i,
    msi: /\.msi$/i,
    appimage: /amd64\.AppImage$/i,
    deb: /amd64\.deb$/i,
    rpm: /x86_64\.rpm$/i,
  };
  var PRIMARY = { mac: "dmg", windows: "exe", linux: "appimage" };

  function detect() {
    var p = ((navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "").toLowerCase();
    var ua = navigator.userAgent.toLowerCase();
    if (/iphone|ipad|android/.test(ua)) return null; // phones: show every option
    if (p.indexOf("mac") >= 0 || ua.indexOf("mac os") >= 0) return "mac";
    if (p.indexOf("win") >= 0 || ua.indexOf("windows") >= 0) return "windows";
    if (p.indexOf("linux") >= 0 || ua.indexOf("linux") >= 0 || ua.indexOf("x11") >= 0) return "linux";
    return null;
  }

  var os = detect();

  // Tabs
  var tabs = document.querySelectorAll(".tabs [data-os]");
  function select(which) {
    tabs.forEach(function (t) {
      var on = t.getAttribute("data-os") === which;
      t.setAttribute("aria-selected", on ? "true" : "false");
    });
    document.querySelectorAll("[data-panel]").forEach(function (p) {
      p.hidden = p.getAttribute("data-panel") !== which;
    });
  }
  tabs.forEach(function (t) {
    t.addEventListener("click", function () { select(t.getAttribute("data-os")); });
  });
  select(os || "mac");

  // Hero button + one-liner
  var label = document.getElementById("download-label");
  var heroCmd = document.getElementById("hero-cmd");
  var note = document.getElementById("download-note");
  if (os && label) label.textContent = "Download for " + NAMES[os];
  if (os && heroCmd) heroCmd.textContent = CMD[os];
  if (os && note) {
    note.innerHTML = os === "windows"
      ? "Paste in <b>PowerShell</b>, then run <code>opencue</code> · <a href=\"#install\">other systems</a>"
      : "Paste in a <b>terminal</b>, then run <code>opencue</code> · <a href=\"#install\">other systems</a>";
  }

  // Copy buttons
  document.querySelectorAll("[data-copy]").forEach(function (b) {
    b.addEventListener("click", function () {
      var el = document.querySelector(b.getAttribute("data-copy"));
      if (!el) return;
      var done = function () {
        var old = b.textContent;
        b.textContent = "Copied";
        b.classList.add("ok");
        setTimeout(function () { b.textContent = old; b.classList.remove("ok"); }, 1600);
      };
      if (navigator.clipboard) navigator.clipboard.writeText(el.textContent.trim()).then(done, function () {});
    });
  });

  // Installer links from the latest release
  fetch("https://api.github.com/repos/" + REPO + "/releases/latest", { headers: { Accept: "application/vnd.github+json" } })
    .then(function (r) { return r.ok ? r.json() : null; })
    .then(function (rel) {
      if (!rel || !rel.assets) return;
      var urls = {};
      Object.keys(PATTERNS).forEach(function (k) {
        var a = rel.assets.find(function (x) { return PATTERNS[k].test(x.name); });
        if (a) urls[k] = a.browser_download_url;
      });
      document.querySelectorAll("[data-asset]").forEach(function (a) {
        var u = urls[a.getAttribute("data-asset")];
        if (u) a.href = u;
      });
      var btn = document.getElementById("download-btn");
      if (os && btn && urls[PRIMARY[os]]) btn.href = urls[PRIMARY[os]];
      var line = document.getElementById("release-line");
      if (line && rel.tag_name) {
        line.innerHTML = "Latest: <a href=\"" + rel.html_url + "\">OpenCue " + rel.tag_name + "</a> · <a href=\"changelog/\">what's new</a> · <a href=\"docs/#install\">all install options</a>";
      }
    })
    .catch(function () {});
})();
