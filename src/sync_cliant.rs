#![allow(unused)]
use std::borrow::Cow;
use std::{collections::HashMap, time::Duration};

use crate::types::HttpClientConfig;
use anyhow::{Context, Error, Result};
use colored::Colorize;
use rayon;
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::cookie::Cookie;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_DISPOSITION, CONTENT_TYPE, RANGE};
use reqwest::{redirect::Policy, NoProxy, Proxy, Url};