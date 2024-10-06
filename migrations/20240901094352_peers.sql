CREATE TABLE peers (
  peer_id VARCHAR(255) NOT NULL PRIMARY KEY,
  name VARCHAR(255) NOT NULL
);

CREATE TABLE tag_levels (
  id INTEGER NOT NULL PRIMARY KEY,
  parent INTEGER,
  name VARCHAR(255) NOT NULL,
  FOREIGN KEY (parent) REFERENCES tag_levels(id)
);

CREATE TABLE tags (
  id INTEGER NOT NULL PRIMARY KEY,
  level INTEGER NOT NULL,
  name VARCHAR(255) NOT NULL,
  FOREIGN KEY (level) REFERENCES tag_levels(id)
);

CREATE TABLE peer_tags (
  peer_id VARCHAR(255) NOT NULL,
  tag_id INTEGER NOT NULL,
  FOREIGN KEY (peer_id) REFERENCES peers(peer_id),
  FOREIGN KEY (tag_id) REFERENCES tags(id),
  PRIMARY KEY (peer_id, tag_id)
);
