// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use crate::utils::{parse_identifiers_normalized, quote_identifier};
use std::borrow::Cow;

/// A resolved path to a table of the form "catalog.schema.table"
#[derive(Debug, Clone)]
pub struct ResolvedTableReference<'a> {
    /// The catalog (aka database) containing the table
    pub catalog: Cow<'a, str>,
    /// The schema containing the table
    pub schema: Cow<'a, str>,
    /// The table name
    pub table: Cow<'a, str>,
}

impl<'a> std::fmt::Display for ResolvedTableReference<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.catalog, self.schema, self.table)
    }
}

/// Represents a path to a table that may require further resolution
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TableReference<'a> {
    /// An unqualified table reference, e.g. "table"
    Bare {
        /// The table name
        table: Cow<'a, str>,
    },
    /// A partially resolved table reference, e.g. "schema.table"
    Partial {
        /// The schema containing the table
        schema: Cow<'a, str>,
        /// The table name
        table: Cow<'a, str>,
    },
    /// A fully resolved table reference, e.g. "catalog.schema.table"
    Full {
        /// The catalog (aka database) containing the table
        catalog: Cow<'a, str>,
        /// The schema containing the table
        schema: Cow<'a, str>,
        /// The table name
        table: Cow<'a, str>,
    },
}

pub type OwnedTableReference = TableReference<'static>;

impl std::fmt::Display for TableReference<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableReference::Bare { table } => write!(f, "{table}"),
            TableReference::Partial { schema, table } => {
                write!(f, "{schema}.{table}")
            }
            TableReference::Full {
                catalog,
                schema,
                table,
            } => write!(f, "{catalog}.{schema}.{table}"),
        }
    }
}

impl<'a> TableReference<'a> {
    /// Convenience method for creating a typed none `None`
    pub fn none() -> Option<TableReference<'a>> {
        None
    }

    /// Convenience method for creating a `Bare` variant of `TableReference`
    pub fn bare(table: impl Into<Cow<'a, str>>) -> TableReference<'a> {
        TableReference::Bare {
            table: table.into(),
        }
    }

    /// Convenience method for creating a `Partial` variant of `TableReference`
    pub fn partial(
        schema: impl Into<Cow<'a, str>>,
        table: impl Into<Cow<'a, str>>,
    ) -> TableReference<'a> {
        TableReference::Partial {
            schema: schema.into(),
            table: table.into(),
        }
    }

    /// Convenience method for creating a `Full` variant of `TableReference`
    pub fn full(
        catalog: impl Into<Cow<'a, str>>,
        schema: impl Into<Cow<'a, str>>,
        table: impl Into<Cow<'a, str>>,
    ) -> TableReference<'a> {
        TableReference::Full {
            catalog: catalog.into(),
            schema: schema.into(),
            table: table.into(),
        }
    }

    /// Retrieve the actual table name, regardless of qualification
    pub fn table(&self) -> &str {
        match self {
            Self::Full { table, .. }
            | Self::Partial { table, .. }
            | Self::Bare { table } => table,
        }
    }

    /// Retrieve the schema name if in the `Partial` or `Full` qualification
    pub fn schema(&self) -> Option<&str> {
        match self {
            Self::Full { schema, .. } | Self::Partial { schema, .. } => Some(schema),
            _ => None,
        }
    }

    /// Retrieve the catalog name if in the `Full` qualification
    pub fn catalog(&self) -> Option<&str> {
        match self {
            Self::Full { catalog, .. } => Some(catalog),
            _ => None,
        }
    }

    /// Compare with another `TableReference` as if both are resolved.
    /// This allows comparing across variants, where if a field is not present
    /// in both variants being compared then it is ignored in the comparison.
    ///
    /// e.g. this allows a `TableReference::Bare` to be considered equal to a
    /// fully qualified `TableReference::Full` if the table names match.
    pub fn resolved_eq(&self, other: &Self) -> bool {
        match self {
            TableReference::Bare { table } => table == other.table(),
            TableReference::Partial { schema, table } => {
                table == other.table() && other.schema().map_or(true, |s| s == schema)
            }
            TableReference::Full {
                catalog,
                schema,
                table,
            } => {
                table == other.table()
                    && other.schema().map_or(true, |s| s == schema)
                    && other.catalog().map_or(true, |c| c == catalog)
            }
        }
    }

    /// Given a default catalog and schema, ensure this table reference is fully resolved
    pub fn resolve(
        self,
        default_catalog: &'a str,
        default_schema: &'a str,
    ) -> ResolvedTableReference<'a> {
        match self {
            Self::Full {
                catalog,
                schema,
                table,
            } => ResolvedTableReference {
                catalog,
                schema,
                table,
            },
            Self::Partial { schema, table } => ResolvedTableReference {
                catalog: default_catalog.into(),
                schema,
                table,
            },
            Self::Bare { table } => ResolvedTableReference {
                catalog: default_catalog.into(),
                schema: default_schema.into(),
                table,
            },
        }
    }

    /// Converts directly into an [`OwnedTableReference`]
    pub fn to_owned_reference(&self) -> OwnedTableReference {
        match self {
            Self::Full {
                catalog,
                schema,
                table,
            } => OwnedTableReference::Full {
                catalog: catalog.to_string().into(),
                schema: schema.to_string().into(),
                table: table.to_string().into(),
            },
            Self::Partial { schema, table } => OwnedTableReference::Partial {
                schema: schema.to_string().into(),
                table: table.to_string().into(),
            },
            Self::Bare { table } => OwnedTableReference::Bare {
                table: table.to_string().into(),
            },
        }
    }

    /// Forms a string where the identifiers are quoted
    pub fn to_quoted_string(&self) -> String {
        match self {
            TableReference::Bare { table } => quote_identifier(table),
            TableReference::Partial { schema, table } => {
                format!("{}.{}", quote_identifier(schema), quote_identifier(table))
            }
            TableReference::Full {
                catalog,
                schema,
                table,
            } => format!(
                "{}.{}.{}",
                quote_identifier(catalog),
                quote_identifier(schema),
                quote_identifier(table)
            ),
        }
    }

    /// Forms a [`TableReference`] by attempting to parse `s` as a multipart identifier,
    /// failing that then taking the entire unnormalized input as the identifier itself.
    ///
    /// Will normalize (convert to lowercase) any unquoted identifiers.
    ///
    /// e.g. `Foo` will be parsed as `foo`, and `"Foo"".bar"` will be parsed as
    /// `Foo".bar` (note the preserved case and requiring two double quotes to represent
    /// a single double quote in the identifier)
    pub fn parse_str(s: &'a str) -> Self {
        let mut parts = parse_identifiers_normalized(s);

        match parts.len() {
            1 => Self::Bare {
                table: parts.remove(0).into(),
            },
            2 => Self::Partial {
                schema: parts.remove(0).into(),
                table: parts.remove(0).into(),
            },
            3 => Self::Full {
                catalog: parts.remove(0).into(),
                schema: parts.remove(0).into(),
                table: parts.remove(0).into(),
            },
            _ => Self::Bare { table: s.into() },
        }
    }
}

/// Parse a `String` into a OwnedTableReference
impl From<String> for OwnedTableReference {
    fn from(s: String) -> Self {
        TableReference::parse_str(&s).to_owned_reference()
    }
}

impl<'a> From<&'a OwnedTableReference> for TableReference<'a> {
    fn from(value: &'a OwnedTableReference) -> Self {
        match value {
            OwnedTableReference::Bare { table } => TableReference::Bare {
                table: Cow::Borrowed(table),
            },
            OwnedTableReference::Partial { schema, table } => TableReference::Partial {
                schema: Cow::Borrowed(schema),
                table: Cow::Borrowed(table),
            },
            OwnedTableReference::Full {
                catalog,
                schema,
                table,
            } => TableReference::Full {
                catalog: Cow::Borrowed(catalog),
                schema: Cow::Borrowed(schema),
                table: Cow::Borrowed(table),
            },
        }
    }
}

/// Parse a string into a TableReference, normalizing where appropriate
///
/// See full details on [`TableReference::parse_str`]
impl<'a> From<&'a str> for TableReference<'a> {
    fn from(s: &'a str) -> Self {
        Self::parse_str(s)
    }
}

impl<'a> From<&'a String> for TableReference<'a> {
    fn from(s: &'a String) -> Self {
        Self::parse_str(s)
    }
}

impl<'a> From<ResolvedTableReference<'a>> for TableReference<'a> {
    fn from(resolved: ResolvedTableReference<'a>) -> Self {
        Self::Full {
            catalog: resolved.catalog,
            schema: resolved.schema,
            table: resolved.table,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_reference_from_str_normalizes() {
        let expected = TableReference::Full {
            catalog: Cow::Owned("catalog".to_string()),
            schema: Cow::Owned("FOO\".bar".to_string()),
            table: Cow::Owned("table".to_string()),
        };
        let actual = TableReference::from("catalog.\"FOO\"\".bar\".TABLE");
        assert_eq!(expected, actual);

        let expected = TableReference::Partial {
            schema: Cow::Owned("FOO\".bar".to_string()),
            table: Cow::Owned("table".to_string()),
        };
        let actual = TableReference::from("\"FOO\"\".bar\".TABLE");
        assert_eq!(expected, actual);

        let expected = TableReference::Bare {
            table: Cow::Owned("table".to_string()),
        };
        let actual = TableReference::from("TABLE");
        assert_eq!(expected, actual);

        // if fail to parse, take entire input string as identifier
        let expected = TableReference::Bare {
            table: Cow::Owned("TABLE()".to_string()),
        };
        let actual = TableReference::from("TABLE()");
        assert_eq!(expected, actual);
    }
}
