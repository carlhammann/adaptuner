-- This is used by pandoc in the build script to convert markdown links to html links
function Link(el)
  el.target = string.gsub(el.target, "%.md", ".html")
  return el
end
