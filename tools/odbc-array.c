#include <sql.h>
#include <sqlext.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void diagnostics(SQLHANDLE handle, SQLSMALLINT type, const char *operation) {
    SQLSMALLINT record = 1;
    SQLCHAR state[6] = {0};
    SQLINTEGER native = 0;
    SQLCHAR message[2048];
    SQLSMALLINT length = 0;
    fprintf(stderr, "%s failed\n", operation);
    while (SQLGetDiagRec(type, handle, record, state, &native, message, sizeof(message), &length) != SQL_NO_DATA) {
        fprintf(stderr, "  %s/%d: %s\n", state, (int)native, message);
        record++;
    }
}

int main(int argc, char **argv) {
    const char *connection_string = getenv("IRIS_ODBC_CONNECTION_STRING");
    SQLHENV environment = SQL_NULL_HENV;
    SQLHDBC connection = SQL_NULL_HDBC;
    SQLHSTMT statement = SQL_NULL_HSTMT;
    SQLRETURN rc;
    int result = 1;
    if (!connection_string || argc != 2) {
        fputs("Set IRIS_ODBC_CONNECTION_STRING and pass a table name.\n", stderr);
        return 2;
    }
    rc = SQLAllocHandle(SQL_HANDLE_ENV, SQL_NULL_HANDLE, &environment);
    if (!SQL_SUCCEEDED(rc)) return 1;
    SQLSetEnvAttr(environment, SQL_ATTR_ODBC_VERSION, (SQLPOINTER)SQL_OV_ODBC3, 0);
    rc = SQLAllocHandle(SQL_HANDLE_DBC, environment, &connection);
    if (!SQL_SUCCEEDED(rc)) goto done;
    rc = SQLDriverConnect(connection, NULL, (SQLCHAR *)connection_string, SQL_NTS, NULL, 0, NULL, SQL_DRIVER_NOPROMPT);
    if (!SQL_SUCCEEDED(rc)) { diagnostics(connection, SQL_HANDLE_DBC, "SQLDriverConnect"); goto done; }
    rc = SQLAllocHandle(SQL_HANDLE_STMT, connection, &statement);
    if (!SQL_SUCCEEDED(rc)) goto done;
    {
        char sql[512];
        char values[3][100] = {"row 1", "row 2", "row 3"};
        SQLLEN lengths[3] = {5, 5, 5};
        SQLULEN processed = 0;
        SQLUSMALLINT statuses[3] = {0};
        SQLLEN affected = -1;
        SQLULEN array_size = 3;
        snprintf(sql, sizeof(sql), "INSERT INTO %s (Name) VALUES (?)", argv[1]);
        rc = SQLPrepare(statement, (SQLCHAR *)sql, SQL_NTS);
        if (!SQL_SUCCEEDED(rc)) { diagnostics(statement, SQL_HANDLE_STMT, "SQLPrepare"); goto done; }
        rc = SQLSetStmtAttr(statement, SQL_ATTR_PARAMSET_SIZE, (SQLPOINTER)array_size, 0);
        if (!SQL_SUCCEEDED(rc)) { diagnostics(statement, SQL_HANDLE_STMT, "SQL_ATTR_PARAMSET_SIZE"); goto done; }
        rc = SQLSetStmtAttr(statement, SQL_ATTR_PARAMS_PROCESSED_PTR, &processed, 0);
        if (!SQL_SUCCEEDED(rc)) { diagnostics(statement, SQL_HANDLE_STMT, "SQL_ATTR_PARAMS_PROCESSED_PTR"); goto done; }
        rc = SQLSetStmtAttr(statement, SQL_ATTR_PARAM_STATUS_PTR, statuses, 0);
        if (!SQL_SUCCEEDED(rc)) { diagnostics(statement, SQL_HANDLE_STMT, "SQL_ATTR_PARAM_STATUS_PTR"); goto done; }
        rc = SQLBindParameter(statement, 1, SQL_PARAM_INPUT, SQL_C_CHAR, SQL_VARCHAR, 100, 0,
                              values, 100, lengths);
        if (!SQL_SUCCEEDED(rc)) { diagnostics(statement, SQL_HANDLE_STMT, "SQLBindParameter"); goto done; }
        rc = SQLExecute(statement);
        if (!SQL_SUCCEEDED(rc)) { diagnostics(statement, SQL_HANDLE_STMT, "SQLExecute"); goto done; }
        SQLRowCount(statement, &affected);
        printf("processed=%llu affected=%lld statuses=%u,%u,%u\n",
               (unsigned long long)processed, (long long)affected,
               statuses[0], statuses[1], statuses[2]);
    }
    result = 0;
done:
    if (statement != SQL_NULL_HSTMT) SQLFreeHandle(SQL_HANDLE_STMT, statement);
    if (connection != SQL_NULL_HDBC) {
        SQLDisconnect(connection);
        SQLFreeHandle(SQL_HANDLE_DBC, connection);
    }
    if (environment != SQL_NULL_HENV) SQLFreeHandle(SQL_HANDLE_ENV, environment);
    return result;
}
