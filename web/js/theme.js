(function () {
  'use strict';

  var root = document.documentElement;
  var theme = '';
  var lang = 'en';

  try {
    var storedTheme = localStorage.getItem('nf.theme');
    if (storedTheme === 'dark' || storedTheme === 'light') {
      theme = storedTheme;
    }

    var storedLang = localStorage.getItem('nf.lang');
    if (storedLang === 'en' || storedLang === 'ja') {
      lang = storedLang;
    }
  } catch (e) {
    // Storage unavailable (private mode, disabled cookies): fall back to defaults.
  }

  root.dataset.theme = theme;
  root.lang = lang;
})();
