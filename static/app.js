(function () {
  "use strict";

  var form = document.getElementById("clean-form");
  var input = document.getElementById("url-input");
  var submitBtn = document.getElementById("submit-btn");
  var errorPanel = document.getElementById("error");
  var resultPanel = document.getElementById("result");
  var resultUrl = document.getElementById("result-url");
  var resultAction = document.getElementById("result-action");
  var resultChanged = document.getElementById("result-changed");
  var strippedWrap = document.getElementById("stripped-wrap");
  var resultStripped = document.getElementById("result-stripped");
  var copyBtn = document.getElementById("copy-btn");

  var lastCleanUrl = "";

  function hide(el) {
    el.hidden = true;
  }

  function show(el) {
    el.hidden = false;
  }

  function clearChildren(el) {
    while (el.firstChild) {
      el.removeChild(el.firstChild);
    }
  }

  function setText(el, text) {
    clearChildren(el);
    el.appendChild(document.createTextNode(text == null ? "" : String(text)));
  }

  function showError(message) {
    hide(resultPanel);
    setText(errorPanel, message);
    show(errorPanel);
  }

  function showResult(data) {
    hide(errorPanel);
    lastCleanUrl = data.url == null ? "" : String(data.url);
    setText(resultUrl, lastCleanUrl || "(null)");
    setText(resultAction, data.action);
    setText(resultChanged, data.changed ? "yes" : "no");

    var params = Array.isArray(data.stripped_params) ? data.stripped_params : [];
    if (params.length > 0) {
      setText(resultStripped, params.join(", "));
      show(strippedWrap);
    } else {
      hide(strippedWrap);
      setText(resultStripped, "");
    }

    show(resultPanel);
  }

  form.addEventListener("submit", function (event) {
    event.preventDefault();
    var url = (input.value || "").trim();
    if (!url) {
      showError("URL is required.");
      return;
    }

    submitBtn.disabled = true;
    hide(errorPanel);
    hide(resultPanel);

    fetch("/clean", {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json" },
      body: JSON.stringify({ url: url }),
    })
      .then(function (response) {
        return response.json().then(function (body) {
          return { ok: response.ok, status: response.status, body: body };
        });
      })
      .then(function (result) {
        if (!result.ok) {
          var msg =
            (result.body && (result.body.message || result.body.error)) ||
            "Request failed (" + result.status + ").";
          showError(msg);
          return;
        }
        showResult(result.body);
      })
      .catch(function () {
        showError("Could not reach the clean API.");
      })
      .finally(function () {
        submitBtn.disabled = false;
      });
  });

  copyBtn.addEventListener("click", function () {
    if (!lastCleanUrl) {
      return;
    }
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(lastCleanUrl).catch(function () {});
      return;
    }
    var ta = document.createElement("textarea");
    ta.value = lastCleanUrl;
    document.body.appendChild(ta);
    ta.select();
    try {
      document.execCommand("copy");
    } catch (_) {}
    document.body.removeChild(ta);
  });
})();
