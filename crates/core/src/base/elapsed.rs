use crate::base::sentence::Sentence;
use crate::say_n;

/// The largest whole unit that fits, as the interface words it: "3 hours ago".
pub fn ago(seconds: u64) -> Sentence {
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    if minutes < 1 {
        return Sentence::named("just now");
    }
    if minutes < 60 {
        return say_n!(minutes as usize, "{} minute ago", "{} minutes ago", minutes);
    }
    if hours < 48 {
        return say_n!(hours as usize, "{} hour ago", "{} hours ago", hours);
    }
    if days < 60 {
        return say_n!(days as usize, "{} day ago", "{} days ago", days);
    }
    if days < 730 {
        let months = days / 30;
        return say_n!(months as usize, "{} month ago", "{} months ago", months);
    }
    let years = days / 365;
    say_n!(years as usize, "{} year ago", "{} years ago", years)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unit_grows_with_the_time() {
        let english = |seconds| ago(seconds).to_string();
        assert_eq!(english(10), "just now");
        assert_eq!(english(60), "1 minute ago");
        assert_eq!(english(2 * 3600), "2 hours ago");
        assert_eq!(english(3 * 86_400), "3 days ago");
        assert_eq!(english(90 * 86_400), "3 months ago");
        assert_eq!(english(800 * 86_400), "2 years ago");
    }
}
