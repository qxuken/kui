return {
  name = "Watchtower",
  id = "dev.kui.samples.watchtower",
  version = "1.0.0",
  entry = "main.tl",
  size = { w = 1320, h = 880 },
  -- the headline cards wrap two a line and the charts stack as it narrows
  -- (ADR 0069); the page scrolls below the dashboard's own size
  min_size = { w = 520, h = 420 },
}
