
use regex::Regex;

pub fn build_regex() -> Regex {
    Regex::new(r"^(?<stem>(?:.*\D)?)(?<num>\d+)(?<ext>(?:\..*)?)$").unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digit_in_extension() {
        let regex = build_regex();
        let s = "test2.3dv";
        let caps = regex.captures(s).unwrap();
        assert_eq!("test", &caps["stem"]);
        assert_eq!("2", &caps["num"]);
        assert_eq!(".3dv", &caps["ext"]);
    }

    #[test]
    fn basic() {
        let regex = build_regex();
        let s = "test2.txt";
        let caps = regex.captures(s).unwrap();
        assert_eq!("test", &caps["stem"]);
        assert_eq!("2", &caps["num"]);
        assert_eq!(".txt", &caps["ext"]);
    }

    #[test]
    fn many_digits() {
        let regex = build_regex();
        let s = "some1other5test2.3dv";
        let caps = regex.captures(s).unwrap();
        assert_eq!("some1other5test", &caps["stem"]);
        assert_eq!("2", &caps["num"]);
        assert_eq!(".3dv", &caps["ext"]);
    }

    #[test]
    fn no_extension() {
        let regex = build_regex();
        let s = "some1other5test2";
        let caps = regex.captures(s).unwrap();
        assert_eq!("some1other5test", &caps["stem"]);
        assert_eq!("2", &caps["num"]);
        assert_eq!("", &caps["ext"]);
    }

    #[test]
    fn one_period_extension() {
        let regex = build_regex();
        let s = "some1other5test2.";
        let caps = regex.captures(s).unwrap();
        assert_eq!("some1other5test", &caps["stem"]);
        assert_eq!("2", &caps["num"]);
        assert_eq!(".", &caps["ext"]);
    }

    #[test]
    fn start_with_number() {
        let regex = build_regex();
        let s = "0some1other5test2.t";
        let caps = regex.captures(s).unwrap();
        assert_eq!("0some1other5test", &caps["stem"]);
        assert_eq!("2", &caps["num"]);
        assert_eq!(".t", &caps["ext"]);
    }

    #[test]
    fn just_number_with_extension() {
        let regex = build_regex();
        let s = "01.t";
        let caps = regex.captures(s).unwrap();
        assert_eq!("", &caps["stem"]);
        assert_eq!("01", &caps["num"]);
        assert_eq!(".t", &caps["ext"]);
    }

    #[test]
    fn just_number() {
        let regex = build_regex();
        let s = "01";
        let caps = regex.captures(s).unwrap();
        assert_eq!("", &caps["stem"]);
        assert_eq!("01", &caps["num"]);
        assert_eq!("", &caps["ext"]);
    }
}