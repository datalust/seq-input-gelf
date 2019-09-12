/*
Example logging with log4j2 and https://github.com/mp911de/logstash-gelf/
*/

package log4j.gelf;

import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

public class App {
    private static final Logger logger = LogManager.getLogger(App.class);

    public static void main(String[] args) {
        logger.info("Hello, from Java!");
    }
}
