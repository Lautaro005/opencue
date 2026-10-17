// Draws OpenCue avatars (a colored shape with a little face) into <span data-avatar="shape face color size">.
// Same artwork as the desktop app (apps/desktop/src/components/AvatarView.tsx).
(function () {
  var INK = "#1d1d1f";
  function star(cx, cy, o, i) {
    var p = [];
    for (var k = 0; k < 10; k++) {
      var r = k % 2 ? i : o, a = (Math.PI / 5) * k - Math.PI / 2;
      p.push((cx + r * Math.cos(a)).toFixed(1) + "," + (cy + r * Math.sin(a)).toFixed(1));
    }
    return p.join(" ");
  }
  function ring(n, rad, r) {
    var s = "";
    for (var k = 0; k < n; k++) {
      var a = ((Math.PI * 2) / n) * k;
      s += '<circle cx="' + (50 + rad * Math.cos(a)).toFixed(1) + '" cy="' + (50 + rad * Math.sin(a)).toFixed(1) + '" r="' + r + '"/>';
    }
    return s;
  }
  var SHAPES = {
    square: [48, 1, function (f) { return '<rect x="9" y="9" width="82" height="82" rx="20" fill="' + f + '"/>'; }],
    circle: [48, 1, function (f) { return '<circle cx="50" cy="50" r="42" fill="' + f + '"/>'; }],
    bubble: [46, 1, function (f) { return '<path d="M50 8C75 8 92 25 92 49V86Q92 92 86 92H50C25 92 8 75 8 50S25 8 50 8Z" fill="' + f + '"/>'; }],
    heart: [42, 0.9, function (f) { return '<path d="M50 89C22 70 7 55 7 35 7 20 18 10 31 10c8 0 15 4 19 11 4-7 11-11 19-11 13 0 24 10 24 25 0 20-15 35-43 54Z" fill="' + f + '"/>'; }],
    star: [55, 0.78, function (f) { return '<polygon points="' + star(50, 55, 44, 22) + '" fill="' + f + '" stroke="' + f + '" stroke-width="9" stroke-linejoin="round"/>'; }],
    triangle: [64, 0.72, function (f) { return '<polygon points="50,14 90,84 10,84" fill="' + f + '" stroke="' + f + '" stroke-width="12" stroke-linejoin="round"/>'; }],
    drop: [62, 0.85, function (f) { return '<path d="M50 5C61 25 84 42 84 63 84 81 69 94 50 94S16 81 16 63C16 42 39 25 50 5Z" fill="' + f + '"/>'; }],
    flower: [50, 0.85, function (f) { return '<g fill="' + f + '">' + ring(8, 29, 15) + '<circle cx="50" cy="50" r="32"/></g>'; }],
    ghost: [44, 0.9, function (f) { return '<path d="M16 46C16 25 31 9 50 9s34 16 34 37v42l-11-8-12 8-11-8-11 8-11-8-12 8Z" fill="' + f + '" stroke="' + f + '" stroke-width="5" stroke-linejoin="round"/>'; }],
    cloud: [58, 0.85, function (f) { return '<g fill="' + f + '"><circle cx="34" cy="56" r="24"/><circle cx="58" cy="42" r="28"/><circle cx="72" cy="62" r="20"/><rect x="14" y="56" width="74" height="26" rx="13"/></g>'; }],
    bot: [58, 0.9, function (f) { return '<g fill="' + f + '"><rect x="13" y="26" width="74" height="64" rx="18"/><rect x="47" y="10" width="6" height="18" rx="3"/><circle cx="50" cy="10" r="7"/></g>'; }],
    hexagon: [50, 0.95, function (f) { return '<polygon points="50,9 86,29 86,71 50,91 14,71 14,29" fill="' + f + '" stroke="' + f + '" stroke-width="10" stroke-linejoin="round"/>'; }],
  };
  var dot = function (x, y, r) { return '<circle cx="' + x + '" cy="' + y + '" r="' + r + '" fill="' + INK + '"/>'; };
  var line = 'fill="none" stroke="' + INK + '" stroke-width="4.5" stroke-linecap="round"';
  var FACES = {
    dots: '<ellipse cx="-14" cy="0" rx="5" ry="6.5" fill="' + INK + '"/><ellipse cx="14" cy="0" rx="5" ry="6.5" fill="' + INK + '"/>',
    googly: '<circle cx="-14" cy="0" r="10" fill="#fff"/><circle cx="14" cy="0" r="10" fill="#fff"/>' + dot(-12, 1.5, 5) + dot(16, 1.5, 5),
    wink: '<ellipse cx="-14" cy="0" rx="4.5" ry="6" fill="' + INK + '"/><path d="M18 -7 9 0l9 7" ' + line + ' stroke-linejoin="round"/>',
    happy: '<path d="M-20 3q6-9 12 0" ' + line + '/><path d="M8 3q6-9 12 0" ' + line + "/>",
    smile: dot(-13, -3, 5) + dot(13, -3, 5) + '<path d="M-6 8q6 6 12 0" ' + line + "/>",
    stern: dot(-13, 2, 5) + dot(13, 2, 5) + '<path d="M-21 -11 -7 -7" ' + line + '/><path d="M21 -11 7 -7" ' + line + "/>",
    curious: dot(-13, 2, 5) + dot(13, 2, 5) + '<path d="M-20 -10h12" ' + line + '/><path d="M8 -13q6-3 12 0" ' + line + "/>",
    sleepy: '<path d="M-20 0h12" ' + line + '/><path d="M8 0h12" ' + line + "/>",
    surprised: dot(-13, -4, 5) + dot(13, -4, 5) + '<ellipse cx="0" cy="10" rx="4" ry="5" fill="' + INK + '"/>',
    shades: '<g fill="' + INK + '"><rect x="-24" y="-7" width="19" height="13" rx="5"/><rect x="5" y="-7" width="19" height="13" rx="5"/><rect x="-6" y="-4" width="12" height="3.5" rx="1.75"/></g>',
  };
  window.ocAvatar = function (shape, face, color, size) {
    var s = SHAPES[shape] || SHAPES.circle;
    return '<svg width="' + size + '" height="' + size + '" viewBox="0 0 100 100" aria-hidden="true" style="overflow:visible">' + s[2](color) +
      '<g transform="translate(50 ' + s[0] + ") scale(" + s[1] + ')">' + (FACES[face] || FACES.dots) + "</g></svg>";
  };
  document.querySelectorAll("[data-avatar]").forEach(function (el) {
    var p = el.getAttribute("data-avatar").split(" ");
    el.innerHTML = window.ocAvatar(p[0], p[1], p[2], Number(p[3] || 36));
    el.style.display = "inline-block";
    el.style.lineHeight = "0";
  });
  var nav = document.querySelector(".nav");
  if (nav) {
    var onScroll = function () { nav.classList.toggle("scrolled", window.scrollY > 4); };
    window.addEventListener("scroll", onScroll, { passive: true });
    onScroll();
  }
})();
