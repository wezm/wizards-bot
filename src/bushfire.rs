//! Monitor feed of bushfires and post notification for any nearby.

use std::fmt::Formatter;
use std::time::Duration;
use std::{fmt, io};

use roxmltree::Node;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use ureq::Agent;

// NOTE: This URL redirects to the actual feed. It's a permanent redirect to an S3 file but I'm
// not sure I trust the permanence of it.
const FEED_URL: &str = "https://www.qfes.qld.gov.au/data/alerts/bushfireAlert.xml";
const ATOM_NS: &str = "http://www.w3.org/2005/Atom";
const GEORSS_NS: &str = "http://www.georss.org/georss";

pub type LatLong = (f64, f64);

#[derive(PartialEq, Eq, Debug, Hash, Default)]
pub struct EntryId(pub(crate) String);

#[derive(Debug, Default, PartialEq)]
pub struct Entry {
    pub category: Option<String>,
    pub content: Option<String>,
    pub id: EntryId,
    pub published: Option<OffsetDateTime>,
    pub title: Option<String>,
    pub updated: Option<OffsetDateTime>,
    pub point: Option<LatLong>,
}

#[derive(Debug)]
pub enum BushfireError {
    Xml(roxmltree::Error),
    Http(ureq::Error),
    Io(io::Error),
}

/// Check for entries to notify about.
pub fn check(notify_near: LatLong) -> Result<Vec<Entry>, BushfireError> {
    let agent: Agent = ureq::AgentBuilder::new()
        .timeout_read(Duration::from_secs(5))
        .timeout_write(Duration::from_secs(5))
        .build();

    // Fetch the feed
    let body: String = agent.get(FEED_URL).call()?.into_string()?;

    // Parse and note entries that are in range
    let mut notify = Vec::new();
    let doc = roxmltree::Document::parse(&body)?;
    for node in doc.descendants() {
        if node.is_element() && node.has_tag_name((ATOM_NS, "entry")) {
            let entry = Entry::parse(node);
            if entry.near(notify_near) {
                notify.push(entry);
            }
        }
    }

    Ok(notify)
}

impl Entry {
    fn parse(node: Node) -> Entry {
        let mut entry = Entry::default();
        for node in node.descendants() {
            if node.is_element() {
                let tag_name = node.tag_name();
                match (tag_name.name(), tag_name.namespace()) {
                    ("category", Some(ATOM_NS)) => {
                        entry.category = node.attribute("term").map(ToOwned::to_owned);
                    }
                    ("content", Some(ATOM_NS)) => {
                        entry.content = node.text().map(ToOwned::to_owned)
                    }
                    ("id", Some(ATOM_NS)) => {
                        if let Some(text) = node.text() {
                            entry.id = EntryId(text.to_owned());
                        }
                    }
                    ("published", Some(ATOM_NS)) => {
                        if let Some(text) = node.text() {
                            entry.published = OffsetDateTime::parse(text, &Rfc3339).ok();
                        }
                    }
                    ("title", Some(ATOM_NS)) => entry.title = node.text().map(ToOwned::to_owned),
                    ("updated", Some(ATOM_NS)) => {
                        if let Some(text) = node.text() {
                            entry.updated = OffsetDateTime::parse(text, &Rfc3339).ok();
                        }
                    }
                    ("point", Some(GEORSS_NS)) => {
                        if let Some(text) = node.text() {
                            let mut coords = text
                                .trim()
                                .split(' ')
                                .flat_map(|val| val.parse::<f64>().ok());
                            if let (Some(lat), Some(long)) = (coords.next(), coords.next()) {
                                entry.point = Some((lat, long));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        entry
    }

    /// Determine if the point in `self` is near the supplied `reference` point.
    fn near(&self, reference: LatLong) -> bool {
        // If we don't know where this entry is then just assume it is nearby to be safe.
        self.point
            .map_or(true, |point| near(reference, point, ALERT_DISTANCE))
    }
}

/// Distance from reference point to edge of square representing the alert region
///
/// ```text
/// |--------| ALERT_DISTANCE
///
/// +-----------------+
/// |                 |
/// |                 |
/// |        X        |
/// |                 |
/// |                 |
/// +-----------------+
/// ```
const ALERT_DISTANCE: f64 = 30.0;

/// Construct a box around `reference` and then see of it contains `point`.
///
/// This is done crudely and assumes that the offsets applied to the reference point won't
/// under/overflow or cross zero.
fn near(reference: LatLong, point: LatLong, alert_distance: f64) -> bool {
    // 0.1 is 11.1 km https://gis.stackexchange.com/a/8655
    let offset = alert_distance / 111.;
    let top_left = (point.0 - offset, point.1 - offset);
    let bottom_right = (point.0 + offset, point.1 + offset);
    (top_left.0..bottom_right.0).contains(&reference.0)
        && (top_left.1..bottom_right.1).contains(&reference.1)
}

impl From<roxmltree::Error> for BushfireError {
    fn from(err: roxmltree::Error) -> Self {
        BushfireError::Xml(err)
    }
}

impl From<ureq::Error> for BushfireError {
    fn from(err: ureq::Error) -> Self {
        BushfireError::Http(err)
    }
}

impl From<io::Error> for BushfireError {
    fn from(err: io::Error) -> Self {
        BushfireError::Io(err)
    }
}

impl fmt::Display for BushfireError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            BushfireError::Xml(err) => {
                write!(f, "unable to parse XML: {err}")
            }
            BushfireError::Http(err) => {
                write!(f, "HTTP request error: {err}")
            }
            BushfireError::Io(err) => {
                write!(f, "I/O error: {err}")
            }
        }
    }
}

impl std::error::Error for BushfireError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_near() {
        let brisbane = (-27.46844, 153.02334);
        // Ocean View (near Caboolture)
        let ocean_view = (-27.127664662091, 152.87902054721);
        let noosa = (-26.400054, 153.0223421);

        assert!(near(brisbane, ocean_view, 50.));
        assert!(!near(brisbane, noosa, 50.));
    }

    #[test]
    fn parse_entry() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:georss="http://www.georss.org/georss" xmlns="http://www.w3.org/2005/Atom">
    <author>
        <name>Queensland Fire and Emergency Services</name>
    </author>
    <generator uri="http://www.safe.com/products/fme/index.php" version="2022.7.44.22623">FME(R) 2022.1.1.0</generator>
    <id>IF39-1924522</id>
    <link href="https://www.qfes.qld.gov.au"/>
    <subtitle>QFES Bushfire Alerts updated regularly</subtitle>
    <title>QFES Bushfire Alert Feed</title>
    <updated>2023-09-09T10:12:08+10:00</updated>
    <entry>
        <author>
          <name>Queensland Fire and Emergency Services</name>
        </author>
        <category term="Watch and Act"/>
        <content>A large fire is burning in the Kumbarilla State Forest and Dunmore State Forest. It is travelling towards Wilkin Road within the Dunmore State Forest.

          Conditions could get worse quickly.

          Firefighters are working to contain the fire. You should not expect a firefighter at your door. Firefighting aircraft are helping ground crews.

          If your life is in danger, call Triple Zero (000) immediately.</content>
        <id>IF39-1919322</id>
        <published>2023-09-08T17:12:08+10:00</published>
        <title>PREPARE TO LEAVE - Cecil Plains and Dunmore (near Kumbarilla) - fire as at  3:52pm Friday,  8 September 2023</title>
        <updated>2023-09-08T15:41:00+10:00</updated>
        <georss:point>-27.584701903466 151.06082028616</georss:point>
    </entry>
</feed>"#;

        let expected = Entry {
            category: Some("Watch and Act".to_string()),
            content: Some("A large fire is burning in the Kumbarilla State Forest and Dunmore State Forest. It is travelling towards Wilkin Road within the Dunmore State Forest.

          Conditions could get worse quickly.

          Firefighters are working to contain the fire. You should not expect a firefighter at your door. Firefighting aircraft are helping ground crews.

          If your life is in danger, call Triple Zero (000) immediately.".to_string()),
            id: EntryId("IF39-1919322".to_string()),
            published: Some(OffsetDateTime::parse("2023-09-08T17:12:08+10:00", &Rfc3339).unwrap()),
            title: Some("PREPARE TO LEAVE - Cecil Plains and Dunmore (near Kumbarilla) - fire as at  3:52pm Friday,  8 September 2023".to_string()),
            updated: Some(OffsetDateTime::parse("2023-09-08T15:41:00+10:00", &Rfc3339).unwrap()),
            point: Some((-27.584701903466, 151.06082028616)),
        };

        let doc = roxmltree::Document::parse(&xml).unwrap();
        for node in doc.descendants() {
            if node.is_element() && node.has_tag_name((ATOM_NS, "entry")) {
                let entry = Entry::parse(node);
                assert_eq!(entry, expected);
            }
        }
    }
}
