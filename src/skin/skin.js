if (typeof KYBER_CSS !== "string" || typeof KYBER_LOGO !== "string") {
  return;
}
if (location.hostname !== "127.0.0.1") {
  return;
}

var crystal = typeof KYBER_CRYSTAL === "string" ? KYBER_CRYSTAL : KYBER_LOGO;

var scheduled = false;

function pinStyle() {
  var style = document.getElementById("kyber-skin");
  if (!style) {
    style = document.createElement("style");
    style.id = "kyber-skin";
    style.textContent = KYBER_CSS;
  }
  var host = document.head || document.documentElement;
  if (style.parentNode !== host || host.lastElementChild !== style) {
    host.appendChild(style);
  }
}

function ensureBoot() {
  if (document.getElementById("kyber-boot") || appReady()) {
    return;
  }
  var host = document.body || document.documentElement;
  var boot = document.createElement("div");
  boot.id = "kyber-boot";
  boot.setAttribute("aria-hidden", "true");
  var img = document.createElement("img");
  img.className = "kyber-boot-mark";
  img.src = crystal;
  img.alt = "";
  boot.appendChild(img);
  host.appendChild(boot);
}

function appReady() {
  return !!(
    document.querySelector('[class*="centerCol"]') ||
    document.querySelector('[class*="sidebarCol"]')
  );
}

function dismissBoot() {
  var boot = document.getElementById("kyber-boot");
  if (boot && appReady()) {
    boot.remove();
  }
}

function forceDark() {
  document.documentElement.style.colorScheme = "dark";
  document.documentElement.style.background = "#07070b";
  if (!document.body) {
    return;
  }
  document.body.setAttribute("data-ds-dark-theme", "");
  document.body.style.colorScheme = "dark";
}

function markImg(className, alt, src) {
  var img = document.createElement("img");
  img.className = className;
  img.src = src;
  img.alt = alt;
  return img;
}

function ensureMark(host, className, src) {
  if (!host) {
    return;
  }
  var existing = host.querySelector("img." + className);
  if (existing) {
    if (existing.src !== src) {
      existing.src = src;
    }
    return;
  }
  host.appendChild(markImg(className, "Kyber Code", src));
}

var dashObserved = typeof WeakSet === "function" ? new WeakSet() : null;
var dashResize =
  typeof ResizeObserver === "function"
    ? new ResizeObserver(function (entries) {
        for (var i = 0; i < entries.length; i += 1) {
          syncCardDash(entries[i].target);
        }
      })
    : null;

function syncCardDash(card) {
  var svg = card.querySelector(":scope > .kyber-card-dash");
  if (!svg) {
    return;
  }
  var rect = svg.firstElementChild;
  var w = card.clientWidth;
  var h = card.clientHeight;
  var sw = 1.5;
  var radius = parseFloat(getComputedStyle(card).borderTopLeftRadius) || 28;
  var rr = Math.max(0, radius - sw / 2);
  svg.setAttribute("viewBox", "0 0 " + w + " " + h);
  rect.setAttribute("x", String(sw / 2));
  rect.setAttribute("y", String(sw / 2));
  rect.setAttribute("width", String(Math.max(0, w - sw)));
  rect.setAttribute("height", String(Math.max(0, h - sw)));
  rect.setAttribute("rx", String(rr));
  rect.setAttribute("ry", String(rr));
}

function ensureCardDash(card) {
  if (!card.querySelector(":scope > .kyber-card-dash")) {
    var svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("class", "kyber-card-dash");
    svg.setAttribute("aria-hidden", "true");
    var rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
    svg.appendChild(rect);
    card.appendChild(svg);
  }
  if (dashResize && dashObserved && !dashObserved.has(card)) {
    dashObserved.add(card);
    dashResize.observe(card);
  }
  syncCardDash(card);
}

function ensureStarfield() {
  var col = document.querySelector('[class*="centerCol"]');
  if (!col || col.querySelector(":scope > .kyber-stars")) {
    return;
  }
  var field = document.createElement("div");
  field.className = "kyber-stars";
  field.setAttribute("aria-hidden", "true");
  var seed = 16807;
  function rnd() {
    seed = (seed * 48271) % 2147483647;
    return (seed - 1) / 2147483646;
  }
  for (var i = 0; i < 72; i += 1) {
    var star = document.createElement("span");
    var kind = rnd();
    var size = kind > 0.9 ? 2.2 : 0.8 + rnd() * 1.4;
    star.className =
      "kyber-star" + (kind > 0.92 ? " kyber-star-warm" : kind > 0.8 ? " kyber-star-cool" : "");
    star.style.cssText =
      "left:" +
      (rnd() * 100).toFixed(2) +
      "%;top:" +
      (rnd() * 100).toFixed(2) +
      "%;width:" +
      size.toFixed(2) +
      "px;height:" +
      size.toFixed(2) +
      "px;animation-delay:" +
      (rnd() * 8).toFixed(2) +
      "s;animation-duration:" +
      (2.8 + rnd() * 4.5).toFixed(2) +
      "s";
    field.appendChild(star);
  }
  col.insertBefore(field, col.firstChild);
}

function clickLabeled(root, pattern) {
  if (!root) {
    return false;
  }
  var buttons = root.querySelectorAll("button");
  for (var i = 0; i < buttons.length; i += 1) {
    if (pattern.test((buttons[i].textContent || "").trim())) {
      buttons[i].click();
      return true;
    }
  }
  return false;
}

function officialDeepSeekRow() {
  var rows = document.querySelectorAll('[class*="rowCard"]');
  for (var i = 0; i < rows.length; i += 1) {
    var row = rows[i];
    var name = row.querySelector('[class*="rowName"]');
    var missing = row.querySelector('[class*="credentialDotMissing"]');
    var custom = row.querySelector('[class*="dangerButton"]');
    if (
      name &&
      missing &&
      !custom &&
      /^DeepSeek$/i.test((name.textContent || "").trim())
    ) {
      return row;
    }
  }
  return null;
}

function flattenOfficialDefault() {
  var official = document.querySelector('[class*="GL8Viq_description"]');
  if (official && /DeepSeek|官方/.test(official.textContent || "")) {
    clickLabeled(official.closest('[class*="dialog"], [class*="content"]') || document, /^(Configure later|稍后配置)$/);
  }
  var setup = document.querySelector('[class*="setupCard"]');
  if (setup && /DeepSeek/i.test(setup.textContent || "")) {
    clickLabeled(setup, /^(Cancel|取消)$/);
  }
  var row = officialDeepSeekRow();
  if (row) {
    if (row.querySelector("input, textarea")) {
      delete row.dataset.kyberRevealing;
      row.removeAttribute("data-kyber-unadded-deepseek");
    } else if (row.dataset.kyberRevealing === "1") {
      row.removeAttribute("data-kyber-unadded-deepseek");
    } else {
      row.setAttribute("data-kyber-unadded-deepseek", "");
    }
  }
  var select = document.querySelector('[class*="addCard"] select');
  if (!select || !officialDeepSeekRow()) {
    return;
  }
  if (
    ![].some.call(select.options, function (option) {
      return option.value === "deepseek-official" || /^DeepSeek$/i.test(option.textContent || "");
    })
  ) {
    var option = document.createElement("option");
    option.value = "deepseek-official";
    option.textContent = "DeepSeek";
    select.appendChild(option);
  }
  if (select.dataset.kyberDeepseekBound) {
    return;
  }
  select.dataset.kyberDeepseekBound = "1";
  select.addEventListener("change", function () {
    if (select.value !== "deepseek-official") {
      return;
    }
    var hidden = officialDeepSeekRow();
    var addCard = select.closest('[class*="addCard"]');
    if (addCard) {
      clickLabeled(addCard, /^(Cancel|取消)$/);
    }
    if (!hidden) {
      return;
    }
    hidden.dataset.kyberRevealing = "1";
    hidden.removeAttribute("data-kyber-unadded-deepseek");
    clickLabeled(hidden, /^(Edit|编辑)$/);
  });
}

function rememberDeepSeekIfConfigured() {
  document.querySelectorAll('[class*="rowCard"]').forEach(function (row) {
    var name = row.querySelector('[class*="rowName"]');
    var ok = row.querySelector('[class*="credentialDotConfigured"]');
    if (name && ok && /^DeepSeek$/i.test((name.textContent || "").trim())) {
      try {
        localStorage.setItem("kyber-deepseek-added", "1");
      } catch (error) {}
    }
  });
}

function deepSeekProviderAdded() {
  try {
    return localStorage.getItem("kyber-deepseek-added") === "1";
  } catch (error) {
    return false;
  }
}

function modelPicked() {
  try {
    return sessionStorage.getItem("kyber-model-picked") === "1";
  } catch (error) {
    return false;
  }
}

function officialDeepSeekName(text) {
  return /^(DeepSeek|深度求索)$/i.test((text || "").trim());
}

function setFlag(el, name, on) {
  if (!el) {
    return;
  }
  if (on) {
    el.setAttribute(name, "");
  } else {
    el.removeAttribute(name);
  }
}

function hushUnaddedDeepSeekModels() {
  rememberDeepSeekIfConfigured();
  var hideOfficial = !deepSeekProviderAdded();
  var hushSeat = hideOfficial && !modelPicked();

  document.querySelectorAll("[class*='_7KE1Ra_root']").forEach(function (root) {
    var label = root.querySelector("[class*='_7KE1Ra_triggerLabel']");
    setFlag(root, "data-kyber-hush-model", hushSeat && label && /DeepSeek/i.test(label.textContent || ""));
  });

  document.querySelectorAll("[class*='_7KE1Ra_groupTitle']").forEach(function (title) {
    setFlag(title.closest("[role='group']"), "data-kyber-unadded-group", hideOfficial && officialDeepSeekName(title.textContent));
  });

  document.querySelectorAll("[class*='_7KE1Ra_cellValue']").forEach(function (value) {
    setFlag(value, "data-kyber-hush-value", hushSeat && /DeepSeek/i.test(value.textContent || ""));
  });

  document.querySelectorAll("[class*='_7KE1Ra_cellLabel']").forEach(function (label) {
    setFlag(
      label.parentElement,
      "data-kyber-hush-effort",
      hushSeat && /^(Effort|推理等级)$/.test((label.textContent || "").trim())
    );
  });

  document.querySelectorAll('[role="listbox"] [role="option"]').forEach(function (row) {
    var list = row.closest('[role="listbox"]');
    var detail = row.querySelector("[class*='detail']");
    var group = ((detail && detail.textContent) || "").split("·")[0];
    setFlag(
      row,
      "data-kyber-unadded-group",
      hideOfficial && list && /\/model/i.test(list.getAttribute("aria-label") || "") && officialDeepSeekName(group)
    );
  });

  document.querySelectorAll("[class*='_7KE1Ra_menu']").forEach(function (menu) {
    var groupsBox = menu.querySelector("[class*='_7KE1Ra_groups']");
    var note = menu.querySelector(".kyber-model-empty");
    if (!groupsBox) {
      if (note) {
        note.remove();
      }
      return;
    }
    var shown = 0;
    var sections = groupsBox.children;
    for (var i = 0; i < sections.length; i += 1) {
      if (!sections[i].hasAttribute("data-kyber-unadded-group")) {
        shown += 1;
      }
    }
    if (hideOfficial && shown === 0) {
      if (!note) {
        note = document.createElement("div");
        note.className = "kyber-model-empty";
        note.textContent = "Add a provider in Models to choose a model.";
        groupsBox.insertAdjacentElement("afterend", note);
      }
    } else if (note) {
      note.remove();
    }
  });
}

function rewriteProviderCopy() {
  var replacements = [
    [
      "Configure the official DeepSeek provider to start building.",
      "Add an API key in Models, or sign in to Codex under Plugins → Codex Connect."
    ],
    [
      "配置 DeepSeek 官方模型，即可开始使用。",
      "在模型设置中添加 API 密钥，或到插件 → Codex Connect 登录 ChatGPT。"
    ],
    [
      "Enter your API keys to use models from the following providers.",
      "Add an API key in Models, or sign in to Codex under Plugins → Codex Connect."
    ],
    [
      "填入各提供方的 API 密钥即可使用其模型。",
      "在模型设置中添加 API 密钥，或到插件 → Codex Connect 登录 ChatGPT。"
    ]
  ];
  document.querySelectorAll("p, h2").forEach(function (el) {
    if (el.childElementCount > 0) {
      return;
    }
    var text = el.textContent || "";
    for (var i = 0; i < replacements.length; i += 1) {
      if (text === replacements[i][0]) {
        el.textContent = replacements[i][1];
        return;
      }
    }
  });
}

function syncWindowDrag() {
  var drag = document.getElementById("kyber-window-drag");
  if (!drag) {
    return;
  }
  var center = document.querySelector('[class*="centerCol"]');
  var titleRow = document.querySelector('[class*="titleRow"]');
  var headerLive = titleRow && titleRow.offsetParent !== null;
  if (center) {
    var box = center.getBoundingClientRect();
    drag.style.left = Math.max(86, Math.round(box.left)) + "px";
    drag.style.right = "0";
    drag.style.width = "auto";
  } else {
    drag.style.left = "86px";
    drag.style.right = "0";
    drag.style.width = "auto";
  }
  drag.style.height = headerLive ? "22px" : "52px";
}

function startWindowDrag(event) {
  if (event.button !== 0 || event.detail !== 1) {
    return;
  }
  var hit = event.target;
  if (!hit.closest) {
    return;
  }
  if (hit.closest("button, a, input, textarea, select, [role='button'], [class*='toggle']")) {
    return;
  }
  var region = hit.closest("[data-tauri-drag-region]");
  if (!region || region.getAttribute("data-tauri-drag-region") === "false") {
    return;
  }
  var internals = window.__TAURI_INTERNALS__;
  if (!internals || !internals.invoke) {
    return;
  }
  event.preventDefault();
  internals.invoke("plugin:window|start_dragging");
}

function rewriteHost(el) {
  if (!el) {
    return;
  }
  var next = (el.textContent || "")
    .replace(/DeepSeek Harness/g, "Kyber Code")
    .replace(/DSH Local Build/g, "Kyber Code")
    .replace(/DeepSeek/g, "Kyber Code");
  if (next !== el.textContent) {
    el.textContent = next;
  }
}

function dress() {
  document.querySelectorAll('[class*="brandMark"], [class*="railMark"]').forEach(function (host) {
    ensureMark(host, "kyber-mark", crystal);
  });
  document
    .querySelectorAll('[class*="headline"]:not([class*="headlineText"])')
    .forEach(function (headline) {
      var hero = headline.querySelector(":scope > .kyber-hero-mark");
      if (!hero) {
        headline.insertBefore(markImg("kyber-hero-mark", "Kyber Code", crystal), headline.firstChild);
        return;
      }
      if (hero.src !== crystal) {
        hero.src = crystal;
      }
    });
  document.querySelectorAll('[class*="logoRow"], [class*="titleRow"]').forEach(function (row) {
    row.setAttribute("data-tauri-drag-region", "deep");
    if (row.matches('[class*="titleRow"]') && row.parentElement) {
      row.parentElement.setAttribute("data-tauri-drag-region", "deep");
    }
    var pad = row.querySelector(".kyber-drag-pad");
    if (row.matches('[class*="logoRow"]')) {
      if (pad) {
        pad.remove();
      }
      return;
    }
    if (pad) {
      return;
    }
    pad = document.createElement("div");
    pad.className = "kyber-drag-pad";
    pad.setAttribute("data-tauri-drag-region", "deep");
    row.insertBefore(pad, row.firstChild);
  });
  document.querySelectorAll('[class*="cardWorkspaceTrigger"]').forEach(ensureCardDash);
  ensureStarfield();
  flattenOfficialDefault();
  hushUnaddedDeepSeekModels();
  rewriteProviderCopy();
  document
    .querySelectorAll('[class*="fallbackBrandName"], [class*="wordmark"]')
    .forEach(rewriteHost);
  if (/DeepSeek/i.test(document.title) || document.title.trim() === "") {
    document.title = "Kyber Code";
  }
  if (document.body && !document.getElementById("kyber-window-drag")) {
    var drag = document.createElement("div");
    drag.id = "kyber-window-drag";
    drag.setAttribute("data-tauri-drag-region", "deep");
    document.body.appendChild(drag);
  }
  syncWindowDrag();
  dismissBoot();
}

function boot() {
  pinStyle();
  forceDark();
  ensureBoot();
  dress();
}

function scheduleBoot() {
  if (scheduled) {
    return;
  }
  scheduled = true;
  requestAnimationFrame(function () {
    scheduled = false;
    boot();
  });
}

function isKyberNode(node) {
  return (
    node.id === "kyber-skin" ||
    node.id === "kyber-window-drag" ||
    node.id === "kyber-boot" ||
    (node.classList &&
      (node.classList.contains("kyber-mark") ||
        node.classList.contains("kyber-hero-mark") ||
        node.classList.contains("kyber-drag-pad") ||
        node.classList.contains("kyber-card-dash") ||
        node.classList.contains("kyber-stars") ||
        node.classList.contains("kyber-star") ||
        node.classList.contains("kyber-model-empty") ||
        node.classList.contains("kyber-boot-mark")))
  );
}

boot();
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot);
}

if (!window.__KYBER_SKIN__) {
  window.__KYBER_SKIN__ = true;
  document.addEventListener(
    "click",
    function (event) {
      var target = event.target;
      if (target.closest && target.closest("[class*='_7KE1Ra_option']")) {
        try {
          sessionStorage.setItem("kyber-model-picked", "1");
        } catch (error) {}
      }
      var toggle = target.closest && target.closest('[class*="logoRow"] [class*="toggle"]');
      if (!toggle) {
        return;
      }
      toggle.blur();
      toggle.dispatchEvent(new MouseEvent("mouseleave", { bubbles: true }));
    },
    true
  );
  window.addEventListener("resize", syncWindowDrag);
  if (typeof ResizeObserver === "function") {
    new ResizeObserver(syncWindowDrag).observe(document.documentElement);
  }
  document.addEventListener("mousedown", startWindowDrag, true);
  new MutationObserver(function (records) {
    for (var i = 0; i < records.length; i += 1) {
      var nodes = records[i].addedNodes;
      for (var j = 0; j < nodes.length; j += 1) {
        if (!isKyberNode(nodes[j])) {
          scheduleBoot();
          return;
        }
      }
    }
  }).observe(document.documentElement, { subtree: true, childList: true });
}
